//! Indonesian (default) and English strings.

use crate::store::Lang;

pub fn t(lang: Lang, key: &str) -> &'static str {
    let id = lang == Lang::Id;
    match key {
        "title" => "Voltrus Kiosk",
        "setup.title" => {
            if id {
                "Pengaturan kios"
            } else {
                "Kiosk setup"
            }
        }
        "setup.server" => {
            if id {
                "Alamat server Voltrus ERP"
            } else {
                "Voltrus ERP server address"
            }
        }
        "setup.server.hint" => {
            if id {
                "Contoh: https://erp.perusahaan.co.id"
            } else {
                "For example https://erp.company.com"
            }
        }
        "setup.camera" => {
            if id {
                "Kamera (nomor)"
            } else {
                "Camera (number)"
            }
        }
        "setup.lang" => {
            if id {
                "Bahasa"
            } else {
                "Language"
            }
        }
        "setup.register" => {
            if id {
                "Daftarkan kios ini di Voltrus: HR → Kios absensi → Baru, isi nama, Device UID dan Public key di bawah ini."
            } else {
                "Register this kiosk in Voltrus: HR → Kiosks → New, with a name and the Device UID and public key below."
            }
        }
        "setup.uid" => "Device UID",
        "setup.key" => "Public key",
        "setup.copy" => {
            if id {
                "Salin"
            } else {
                "Copy"
            }
        }
        "setup.connect" => {
            if id {
                "Simpan dan hubungkan"
            } else {
                "Save and connect"
            }
        }
        "setup.connecting" => {
            if id {
                "Menghubungkan…"
            } else {
                "Connecting…"
            }
        }
        "setup.back" => {
            if id {
                "Kembali ke kios"
            } else {
                "Back to the kiosk"
            }
        }
        "setup.ok" => {
            if id {
                "Terhubung."
            } else {
                "Connected."
            }
        }
        "setup.shortcut" => {
            if id {
                "Ctrl+Shift+S membuka pengaturan, F11 layar penuh."
            } else {
                "Ctrl+Shift+S opens setup, F11 toggles full screen."
            }
        }
        "mode.auto" => {
            if id {
                "Otomatis"
            } else {
                "Auto"
            }
        }
        "mode.in" => {
            if id {
                "Masuk"
            } else {
                "In"
            }
        }
        "mode.out" => {
            if id {
                "Pulang"
            } else {
                "Out"
            }
        }
        "seen.nobody" => {
            if id {
                "Silakan menghadap kamera"
            } else {
                "Please face the camera"
            }
        }
        "seen.unknown" => {
            if id {
                "Tahan sebentar…"
            } else {
                "Hold still…"
            }
        }
        "seen.notlive" => {
            if id {
                "Wajah asli diperlukan, bukan foto atau layar"
            } else {
                "A real face is needed, not a photo or screen"
            }
        }
        "seen.again" => {
            if id {
                "Sudah tercatat"
            } else {
                "Already recorded"
            }
        }
        "recorded.in" => {
            if id {
                "Selamat datang"
            } else {
                "Welcome"
            }
        }
        "recorded.out" => {
            if id {
                "Hati-hati di jalan"
            } else {
                "See you"
            }
        }
        "recent" => {
            if id {
                "Terakhir"
            } else {
                "Recent"
            }
        }
        "status.online" => {
            if id {
                "Daring"
            } else {
                "Online"
            }
        }
        "status.offline" => {
            if id {
                "Luring, absensi disimpan"
            } else {
                "Offline, saving check-ins"
            }
        }
        "status.queue" => {
            if id {
                "antre"
            } else {
                "queued"
            }
        }
        "status.people" => {
            if id {
                "karyawan"
            } else {
                "employees"
            }
        }
        "status.faces" => {
            if id {
                "wajah"
            } else {
                "faces"
            }
        }
        "status.nogallery" => {
            if id {
                "Belum ada data karyawan: hubungkan ke server"
            } else {
                "No employee data yet: connect to the server"
            }
        }
        "status.camera" => {
            if id {
                "Kamera tidak tersedia"
            } else {
                "Camera unavailable"
            }
        }
        "status.rejected" => {
            if id {
                "ditolak"
            } else {
                "rejected"
            }
        }
        _ => "?",
    }
}
