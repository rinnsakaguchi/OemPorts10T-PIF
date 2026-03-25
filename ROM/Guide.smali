Import dex to settings
Add xml,copy id xml,replace id di smali Lcom/android/settings/overlay/PIFDashboard;

masukam ini di xml mana pun,contoh di xml addon

<com.oplus.settings.widget.preference.SettingsSimpleJumpPreference
            android:title="PIF Addon"
            android:key="pif_dashboard_jump"
            android:fragment="com.android.settings.overlay.PIFDashboard"
            app:layoutCategory="no_icon_and_summary" />