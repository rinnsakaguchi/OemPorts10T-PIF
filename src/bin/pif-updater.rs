use std::env;
use std::fs;
use std::io::{self, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread::sleep;
use std::time::Duration;

const PIF_APK: &str = "/data/system/PIF.apk";
const PIF_INFO_TMP: &str = "/data/system/pif_info_tmp";
const PIF_INFO: &str = "/data/pif_info";
const OEM_DIR: &str = "/data/local/oemports10t";
const URL_BASE: &str = "https://raw.githubusercontent.com/rinnsakaguchi/OemPorts10T-PIF/pif-apk";

fn download_file(url: &str, dest_path: &str) -> Result<(), String> {
    let agent = ureq::AgentBuilder::new().timeout(Duration::from_secs(30)).build();
    let response = agent.get(url).call().map_err(|e| format!("Network error: {}", e))?;
    if response.status() == 200 {
        let mut file = fs::File::create(dest_path).map_err(|e| format!("Failed to create file: {}", e))?;
        let mut reader = response.into_reader();
        io::copy(&mut reader, &mut file).map_err(|e| format!("Write error: {}", e))?;
        Ok(())
    } else {
        Err(format!("Server returned status: {}", response.status()))
    }
}

fn md5sum(path: &str) -> String {
    if !Path::new(path).exists() { return "0".to_string(); }
    let output = Command::new("md5sum").arg(path).output();
    if let Ok(out) = output {
        let text = String::from_utf8_lossy(&out.stdout);
        text.split_whitespace().next().unwrap_or("0").to_string()
    } else {
        "0".to_string()
    }
}

fn silent_kill(process: &str) {
    let _ = Command::new("killall").arg(process).stdout(Stdio::null()).stderr(Stdio::null()).status();
}

fn hellyeah_dir() {
    if !Path::new(OEM_DIR).exists() {
        let _ = fs::create_dir_all(OEM_DIR).ok();
        let _ = fs::set_permissions(OEM_DIR, fs::Permissions::from_mode(0o777));
        let _ = Command::new("chown").args(&["root:root", OEM_DIR]).status();
    }
}

fn push_pif_json(path: &str) {
    hellyeah_dir();
    let dest = format!("{}/pif.json", OEM_DIR);
    if fs::copy(path, &dest).is_ok() {
        let _ = fs::set_permissions(&dest, fs::Permissions::from_mode(0o644));
        let _ = Command::new("chown").args(&["root:root", &dest]).status();
        println!("✓ pif.json injected to {}", OEM_DIR);
    }
}

fn push_keybox_xml(path: &str) {
    hellyeah_dir();
    let dest = format!("{}/keybox.xml", OEM_DIR);
    if fs::copy(path, &dest).is_ok() {
        let _ = fs::set_permissions(&dest, fs::Permissions::from_mode(0o644));
        let _ = Command::new("chown").args(&["root:root", &dest]).status();
        println!("✓ keybox.xml injected to {}", OEM_DIR);
    }
}

// Fungsi untuk FORCE Refresh (Tanpa cek MD5)
fn force_refresh() {
    println!("Checking internet connection...");
    if Command::new("ping").args(&["-c", "1", "-W", "2", "8.8.8.8"]).stdout(Stdio::null()).status().is_ok() {
        println!("Connected! Starting force refresh...");
        
        let tmp_json = "/data/local/tmp/pif_force.json";
        let tmp_kbox = "/data/local/tmp/kbox_force.xml";

        println!("Forcing download of pif.json...");
        if download_file(&format!("{}/pif.json", URL_BASE), tmp_json).is_ok() {
            push_pif_json(tmp_json);
            let _ = fs::remove_file(tmp_json);
        }

        println!("Forcing download of keybox.xml...");
        if download_file(&format!("{}/keybox.xml", URL_BASE), tmp_kbox).is_ok() {
            push_keybox_xml(tmp_kbox);
            let _ = fs::remove_file(tmp_kbox);
        }

        println!("\nRestarting system components...");
        silent_kill("com.google.android.gms.unstable");
        silent_kill("com.android.vending");
        let _ = Command::new("pkill").arg("systemui").status();
        println!("Force refresh completed!");
    } else {
        println!("No internet connection!");
    }
}

fn main_logic() {
    // ... (Logika main_logic tetap sama seperti sebelumnya untuk auto-update normal)
    println!("Checking internet connection...");
    loop {
        if Command::new("ping").args(&["-c", "1", "-W", "2", "8.8.8.8"]).stdout(Stdio::null()).status().map(|s| s.success()).unwrap_or(false) { break; }
        sleep(Duration::from_secs(3));
    }
    let _ = download_file(&format!("{}/info.txt", URL_BASE), PIF_INFO_TMP);
    
    println!("\n+------------------------------+");
    println!("|         PIF.apk INFO         |");
    println!("+------------------------------+");
    if let Ok(content) = fs::read_to_string(PIF_INFO_TMP) { print!("{}", content); }
    println!("--------------------------------");

    let mut updated = false;

    if md5sum(PIF_INFO_TMP) != md5sum(PIF_INFO) {
        println!("New PIF.apk version found!");
        if download_file(&format!("{}/PIF.apk", URL_BASE), PIF_APK).is_ok() {
            let _ = Command::new("pm").args(&["install", "-r", PIF_APK]).status();
            let _ = fs::copy(PIF_INFO_TMP, PIF_INFO);
            updated = true;
        }
    }

    let tmp_json = "/data/local/tmp/pif_remote.json";
    if download_file(&format!("{}/pif.json", URL_BASE), tmp_json).is_ok() {
        if md5sum(tmp_json) != md5sum(&format!("{}/pif.json", OEM_DIR)) {
            push_pif_json(tmp_json);
            updated = true;
        }
        let _ = fs::remove_file(tmp_json);
    }

    let tmp_kbox = "/data/local/tmp/kbox_remote.xml";
    if download_file(&format!("{}/keybox.xml", URL_BASE), tmp_kbox).is_ok() {
        if md5sum(tmp_kbox) != md5sum(&format!("{}/keybox.xml", OEM_DIR)) {
            push_keybox_xml(tmp_kbox);
            updated = true;
        }
        let _ = fs::remove_file(tmp_kbox);
    }

    if updated {
        silent_kill("com.google.android.gms.unstable");
        silent_kill("com.android.vending");
        let _ = Command::new("pkill").arg("systemui").status();
    } else {
        println!("Everything is already the latest version.");
    }
    let _ = fs::remove_file(PIF_INFO_TMP);
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() > 1 {
        match args[1].as_str() {
            "refresh" => force_refresh(), // Command baru pif-updater refresh
            "-p" => if args.len() > 2 { push_pif_json(&args[2]); },
            "-k" => if args.len() > 2 { push_keybox_xml(&args[2]); },
            _ => main_logic(),
        }
    } else {
        main_logic();
    }
}
