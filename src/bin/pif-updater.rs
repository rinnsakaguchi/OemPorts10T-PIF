use std::env;
use std::fs;
use std::io::{self, BufRead};
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread::sleep;
use std::time::Duration;

// --- Konfigurasi Path ---
const PIF_APK: &str = "/data/system/PIF.apk";
const PIF_INFO_TMP: &str = "/data/system/pif_info_tmp";
const PIF_INFO: &str = "/data/pif_info";
const OEM_DIR: &str = "/data/local/oemports10t";

// --- URL Master (GitHub) ---
const INFO_URL: &str = "https://raw.githubusercontent.com/rinnsakaguchi/OemPorts10T-PIF/load/info.txt";
const URL_APK_BRANCH: &str = "https://raw.githubusercontent.com/rinnsakaguchi/OemPorts10T-PIF/pif-apk";

// Fungsi untuk memperbaiki URL
fn fix_pixeldrain_url(url: &str) -> String {
    let trimmed = url.trim();
    if trimmed.contains("pixeldrain.com/u/") {
        trimmed.replace("pixeldrain.com/u/", "pixeldrain.com/api/file/")
    } else {
        trimmed.to_string()
    }
}

fn download_file(url: &str, dest_path: &str) -> Result<(), String> {
    let agent = ureq::AgentBuilder::new().timeout(Duration::from_secs(30)).build();
    let response = agent.get(url).call().map_err(|e| format!("Network error: {}", e))?;
    if response.status() == 200 {
        let mut file = fs::File::create(dest_path).map_err(|e| format!("Failed to create file: {}", e))?;
        let mut reader = response.into_reader();
        io::copy(&mut reader, &mut file).map_err(|e| format!("Write error: {}", e))?;
        Ok(())
    } else {
        Err(format!("Server status: {}", response.status()))
    }
}

fn get_config_value(file_path: &str, key: &str) -> Option<String> {
    if let Ok(file) = fs::File::open(file_path) {
        let reader = io::BufReader::new(file);
        for line in reader.lines().flatten() {
            let lower_line = line.to_lowercase();
            if lower_line.starts_with(&key.to_lowercase()) {
                let parts: Vec<&str> = line.split('=').collect();
                if parts.len() > 1 {
                    return Some(parts[1].trim().to_string());
                }
            }
        }
    }
    None
}

fn md5sum(path: &str) -> String {
    if !Path::new(path).exists() { return "0".to_string(); }
    let output = Command::new("md5sum").arg(path).output();
    if let Ok(out) = output {
        let text = String::from_utf8_lossy(&out.stdout);
        text.split_whitespace().next().unwrap_or("0").to_string()
    } else { "0".to_string() }
}

fn silent_kill(process: &str) {
    let _ = Command::new("killall").arg(process).stdout(Stdio::null()).stderr(Stdio::null()).status();
}

fn ensure_oem_dir() {
    if !Path::new(OEM_DIR).exists() { let _ = fs::create_dir_all(OEM_DIR).ok(); }
    let _ = fs::set_permissions(OEM_DIR, fs::Permissions::from_mode(0o755));
    let _ = Command::new("chown").args(&["root:root", OEM_DIR]).status();
}

fn push_file(src: &str, filename: &str) {
    ensure_oem_dir();
    let dest = format!("{}/{}", OEM_DIR, filename);
    let _ = fs::remove_file(&dest); // Pastikan remove dulu
    if fs::copy(src, &dest).is_ok() {
        let _ = fs::set_permissions(&dest, fs::Permissions::from_mode(0o644));
        let _ = Command::new("chown").args(&["root:root", &dest]).status();
        println!("✓ {} injected to {}", filename, OEM_DIR);
    }
}

fn force_refresh() {
    println!("Checking connection...");
    if Command::new("ping").args(&["-c", "1", "-W", "2", "8.8.8.8"]).stdout(Stdio::null()).status().is_ok() {
        println!("Connected! Fetching links from info.txt...");
        let info_tmp = "/data/local/tmp/info_refresh.txt";

        if download_file(INFO_URL, info_tmp).is_ok() {
            // Ambil URL dan Fix jika formatnya masih /u/
            let pif_url = fix_pixeldrain_url(&get_config_value(info_tmp, "pif").unwrap_or_default());
            let kbox_url = fix_pixeldrain_url(&get_config_value(info_tmp, "keybox").unwrap_or_default());

            if !pif_url.is_empty() && pif_url.contains("http") {
                println!("Refreshing pif.json...");
                let tmp = "/data/local/tmp/pif_f.json";
                if download_file(&pif_url, tmp).is_ok() { push_file(tmp, "pif.json"); let _ = fs::remove_file(tmp); }
            }

            if !kbox_url.is_empty() && kbox_url.contains("http") {
                println!("Refreshing keybox.xml...");
                let tmp = "/data/local/tmp/kbox_f.xml";
                if download_file(&kbox_url, tmp).is_ok() { push_file(tmp, "keybox.xml"); let _ = fs::remove_file(tmp); }
            }

            println!("\nRestarting system components...");
            silent_kill("com.google.android.gms.unstable");
            silent_kill("com.android.vending");
            let _ = Command::new("pkill").arg("-f").arg("com.android.systemui").status();
            let _ = fs::remove_file(info_tmp);
            println!("Force refresh completed!");
        } else { println!("Failed to download info.txt!"); }
    } else { println!("No internet!"); }
}

fn main_logic() {
    println!("Checking connection...");
    loop {
        if Command::new("ping").args(&["-c", "1", "-W", "2", "8.8.8.8"]).status().map(|s| s.success()).unwrap_or(false) { break; }
        sleep(Duration::from_secs(3));
    }

    let info_tmp = "/data/local/tmp/main_info.txt";
    if download_file(INFO_URL, info_tmp).is_ok() {
        let pif_url = fix_pixeldrain_url(&get_config_value(info_tmp, "pif").unwrap_or_default());
        let kbox_url = fix_pixeldrain_url(&get_config_value(info_tmp, "keybox").unwrap_or_default());

        // Update APK
        let _ = download_file(&format!("{}/info.txt", URL_APK_BRANCH), PIF_INFO_TMP);
        if md5sum(PIF_INFO_TMP) != md5sum(PIF_INFO) {
            println!("New PIF.apk found, installing...");
            if download_file(&format!("{}/PIF.apk", URL_APK_BRANCH), PIF_APK).is_ok() {
                let _ = Command::new("pm").args(&["install", "-r", PIF_APK]).status();
                let _ = fs::copy(PIF_INFO_TMP, PIF_INFO);
            }
        }

        // Auto-Update JSON & XML
        let mut updated = false;
        if !pif_url.is_empty() {
            let tmp = "/data/local/tmp/p_rem.json";
            if download_file(&pif_url, tmp).is_ok() {
                if md5sum(tmp) != md5sum(&format!("{}/pif.json", OEM_DIR)) { push_file(tmp, "pif.json"); updated = true; }
                let _ = fs::remove_file(tmp);
            }
        }
        if !kbox_url.is_empty() {
            let tmp = "/data/local/tmp/k_rem.xml";
            if download_file(&kbox_url, tmp).is_ok() {
                if md5sum(tmp) != md5sum(&format!("{}/keybox.xml", OEM_DIR)) { push_file(tmp, "keybox.xml"); updated = true; }
                let _ = fs::remove_file(tmp);
            }
        }

        if updated {
            silent_kill("com.google.android.gms.unstable");
            silent_kill("com.android.vending");
            let _ = Command::new("pkill").arg("-f").arg("com.android.systemui").status();
        }
        let _ = fs::remove_file(info_tmp);
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() > 1 && args[1] == "refresh" {
        force_refresh();
    } else {
        main_logic();
    }
}
