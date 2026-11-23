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
                "Wajah belum dikenali"
            } else {
                "Face not recognised"
            }
        }
        "seen.notlive" => {
            if id {
                "Wajah asli diperlukan"
            } else {
                "A real face is needed"
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
        "update.check" => {
            if id {
                "Cek pembaruan"
            } else {
                "Check for updates"
            }
        }
        "update.checking" => {
            if id {
                "Memeriksa pembaruan…"
            } else {
                "Checking for updates…"
            }
        }
        "update.current" => {
            if id {
                "Sudah versi terbaru."
            } else {
                "This is the newest version."
            }
        }
        "update.ready" => {
            if id {
                "Siap dipasang saat kamera sepi: versi"
            } else {
                "Installs when nobody is at the camera: version"
            }
        }
        "update.failed" => {
            if id {
                "Pembaruan gagal"
            } else {
                "Update failed"
            }
        }
        "update.restart" => {
            if id {
                "Mulai ulang sekarang"
            } else {
                "Restart now"
            }
        }
        "update.auto.on" => {
            if id {
                "Pembaruan otomatis: nyala"
            } else {
                "Auto update: on"
            }
        }
        "update.auto.off" => {
            if id {
                "Pembaruan otomatis: mati"
            } else {
                "Auto update: off"
            }
        }
        "seen.nobody.hint" => {
            if id {
                "Berdiri sekitar satu meter dari kamera"
            } else {
                "Stand about a metre from the camera"
            }
        }
        "seen.matching" => {
            if id {
                "Mengenali wajah…"
            } else {
                "Recognising…"
            }
        }
        "seen.matching.hint" => {
            if id {
                "Tahan posisi sebentar"
            } else {
                "Hold still for a moment"
            }
        }
        "seen.unknown.hint" => {
            if id {
                "Wajah Anda belum terdaftar. Minta HR mendaftarkannya di Voltrus."
            } else {
                "Your face is not enrolled. Ask HR to enrol it in Voltrus."
            }
        }
        "seen.notlive.hint" => {
            if id {
                "Foto atau layar tidak bisa dipakai untuk absen"
            } else {
                "Photos and screens can't be used to check in"
            }
        }
        "seen.again.hint" => {
            if id {
                "Absensi Anda sudah tercatat barusan"
            } else {
                "Your check-in was just recorded"
            }
        }
        "seen.nofaces" => {
            if id {
                "Belum ada wajah terdaftar"
            } else {
                "No faces enrolled yet"
            }
        }
        "seen.nofaces.hint" => {
            if id {
                "Daftarkan wajah karyawan di Voltrus (HR → Karyawan). Kios mengambil data baru tiap 5 menit."
            } else {
                "Enrol employees' faces in Voltrus (HR → Employees). The kiosk pulls new faces every 5 minutes."
            }
        }
        "seen.nogallery" => {
            if id {
                "Kios belum terhubung"
            } else {
                "The kiosk is not connected"
            }
        }
        "recent.empty" => {
            if id {
                "Belum ada absensi"
            } else {
                "No check-ins yet"
            }
        }
        "live" => {
            if id {
                "LANGSUNG"
            } else {
                "LIVE"
            }
        }
        "brand.tagline" => {
            if id {
                "Absensi wajah untuk Voltrus ERP"
            } else {
                "Face attendance for Voltrus ERP"
            }
        }
        "setup.subtitle" => {
            if id {
                "Hubungkan kios ini ke server Voltrus ERP perusahaan Anda."
            } else {
                "Connect this kiosk to your company's Voltrus ERP server."
            }
        }
        "setup.step1" => {
            if id {
                "Isi alamat server"
            } else {
                "Enter the server address"
            }
        }
        "setup.step1.hint" => {
            if id {
                "Alamat yang dipakai untuk membuka Voltrus di browser."
            } else {
                "The address you open Voltrus with in a browser."
            }
        }
        "setup.step2" => {
            if id {
                "Daftarkan kios"
            } else {
                "Register the kiosk"
            }
        }
        "setup.step2.hint" => {
            if id {
                "Di Voltrus: HR → Kios absensi → Baru. Tempel Device UID dan public key."
            } else {
                "In Voltrus: HR → Kiosks → New. Paste the Device UID and public key."
            }
        }
        "setup.step3" => {
            if id {
                "Simpan dan hubungkan"
            } else {
                "Save and connect"
            }
        }
        "setup.step3.hint" => {
            if id {
                "Kios mengunduh model wajah dan data karyawan."
            } else {
                "The kiosk downloads the face models and employees."
            }
        }
        "setup.identity" => {
            if id {
                "Identitas perangkat"
            } else {
                "Device identity"
            }
        }
        "setup.copied" => {
            if id {
                "Tersalin"
            } else {
                "Copied"
            }
        }
        "setup.update" => {
            if id {
                "Pembaruan"
            } else {
                "Updates"
            }
        }
        "setup.shortcuts" => {
            if id {
                "Pintasan"
            } else {
                "Shortcuts"
            }
        }
        "setup.shortcut.setup" => {
            if id {
                "Pengaturan"
            } else {
                "Setup"
            }
        }
        "setup.shortcut.full" => {
            if id {
                "Layar penuh"
            } else {
                "Full screen"
            }
        }
        "update.auto" => {
            if id {
                "Pasang pembaruan otomatis"
            } else {
                "Install updates automatically"
            }
        }
        _ => "?",
    }
}

/// "Senin, 23 November 2026" / "Monday, 23 November 2026".
pub fn long_date(lang: Lang, date: chrono::NaiveDate) -> String {
    use chrono::Datelike as _;
    const DAYS_ID: [&str; 7] = [
        "Senin", "Selasa", "Rabu", "Kamis", "Jumat", "Sabtu", "Minggu",
    ];
    const DAYS_EN: [&str; 7] = [
        "Monday",
        "Tuesday",
        "Wednesday",
        "Thursday",
        "Friday",
        "Saturday",
        "Sunday",
    ];
    const MONTHS_ID: [&str; 12] = [
        "Januari",
        "Februari",
        "Maret",
        "April",
        "Mei",
        "Juni",
        "Juli",
        "Agustus",
        "September",
        "Oktober",
        "November",
        "Desember",
    ];
    const MONTHS_EN: [&str; 12] = [
        "January",
        "February",
        "March",
        "April",
        "May",
        "June",
        "July",
        "August",
        "September",
        "October",
        "November",
        "December",
    ];
    let (days, months) = if lang == Lang::Id {
        (DAYS_ID, MONTHS_ID)
    } else {
        (DAYS_EN, MONTHS_EN)
    };
    format!(
        "{}, {} {} {}",
        days[date.weekday().num_days_from_monday() as usize],
        date.day(),
        months[date.month0() as usize],
        date.year()
    )
}
