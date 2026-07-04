//! Basit kalici vertex deposu: length-prefixed append-log.
//!
//! Format: her kayit = [4 bayt uzunluk (big-endian u32)] + [o kadar bayt vertex].
//! Ham vertex baytlari her degeri (newline dahil) icerebilir; bu yuzden satir
//! bazli degil, uzunluk-onekli ayirma kullaniriz (binary-guvenli, kayipsiz).
//!
//! Tasarim: bu modul AGDAN ve MOTORDAN bagimsizdir (saf dosya I/O). Boylece
//! tek basina test edilebilir. run_node bunu cagirir.

use std::fs::{File, OpenOptions};
use std::io::{self, BufReader, BufWriter, Read, Write};
use std::path::Path;

/// Bir vertex'i (ham bayt) log dosyasina ekler (append).
/// Format: 4 bayt uzunluk (BE) + vertex baytlari. Her cagri dosyayi flush eder
/// (dayaniklilik: cokme aninda yarim yazim riskini azaltir).
pub fn append_vertex(path: &Path, vertex_bytes: &[u8]) -> io::Result<()> {
    let file = OpenOptions::new().create(true).append(true).open(path)?;
    let mut w = BufWriter::new(file);
    let len = vertex_bytes.len() as u32;
    w.write_all(&len.to_be_bytes())?;
    w.write_all(vertex_bytes)?;
    w.flush()?;
    Ok(())
}

/// Log dosyasindaki TUM vertex'leri (ham bayt) okur.
/// Dosya yoksa bos Vec doner (ilk calistirma — hata degil).
/// Bozuk/yarim son kayit (cokme artigi) sessizce atlanir; o ana kadar
/// okunan saglam kayitlar dondurulur (kismi kurtarma).
pub fn load_vertices(path: &Path) -> io::Result<Vec<Vec<u8>>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let file = File::open(path)?;
    let mut r = BufReader::new(file);
    let mut out = Vec::new();

    loop {
        // 4 bayt uzunluk oku.
        let mut len_buf = [0u8; 4];
        match r.read_exact(&mut len_buf) {
            Ok(()) => {}
            Err(ref e) if e.kind() == io::ErrorKind::UnexpectedEof => break, // temiz son
            Err(e) => return Err(e),
        }
        let len = u32::from_be_bytes(len_buf) as usize;

        // O kadar bayt oku. Eksikse (yarim kayit) -> dur, o ana kadarini koru.
        let mut buf = vec![0u8; len];
        match r.read_exact(&mut buf) {
            Ok(()) => out.push(buf),
            Err(ref e) if e.kind() == io::ErrorKind::UnexpectedEof => break, // yarim son kayit
            Err(e) => return Err(e),
        }
    }
    Ok(out)
}

/// Genel amacli: bir bayt dizisini dosyaya yazar (tum dosyayi degistirir, append DEGIL).
/// Dugum imzalama anahtari gibi kucuk, tekil veriler icin.
pub fn save_bytes(path: &Path, data: &[u8]) -> io::Result<()> {
    let mut w = BufWriter::new(File::create(path)?);
    w.write_all(data)?;
    w.flush()?;
    Ok(())
}

/// Genel amacli: dosyadaki tum baytlari okur. Dosya yoksa None.
pub fn load_bytes(path: &Path) -> io::Result<Option<Vec<u8>>> {
    if !path.exists() {
        return Ok(None);
    }
    let mut f = File::open(path)?;
    let mut buf = Vec::new();
    f.read_to_end(&mut buf)?;
    Ok(Some(buf))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    // Test icin gecici, benzersiz dosya yolu (cakisma olmasin).
    fn temp_path(tag: &str) -> PathBuf {
        let mut p = std::env::temp_dir();
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        p.push(format!("lsc-store-test-{tag}-{nanos}.log"));
        p
    }

    #[test]
    fn load_missing_file_is_empty() {
        let p = temp_path("missing");
        let v = load_vertices(&p).expect("load");
        assert!(v.is_empty());
    }

    #[test]
    fn append_then_load_roundtrip() {
        let p = temp_path("roundtrip");
        let a = vec![1u8, 2, 3];
        let b = vec![9u8; 200]; // 200 baytlik kayit
        let c = vec![0u8, 10, 13, 255, 0]; // newline (10) + carriage (13) icerir
        append_vertex(&p, &a).expect("a");
        append_vertex(&p, &b).expect("b");
        append_vertex(&p, &c).expect("c");

        let loaded = load_vertices(&p).expect("load");
        assert_eq!(loaded.len(), 3);
        assert_eq!(loaded[0], a);
        assert_eq!(loaded[1], b);
        assert_eq!(
            loaded[2], c,
            "binary (newline iceren) kayit bozulmadan okundu"
        );

        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn empty_vertex_record_roundtrip() {
        let p = temp_path("empty-rec");
        let empty: Vec<u8> = vec![];
        append_vertex(&p, &empty).expect("empty");
        let loaded = load_vertices(&p).expect("load");
        assert_eq!(loaded.len(), 1);
        assert!(loaded[0].is_empty());
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn truncated_last_record_is_skipped() {
        let p = temp_path("truncated");
        // Once saglam bir kayit yaz.
        append_vertex(&p, &[7u8, 7, 7]).expect("ok rec");
        // Sonra elle BOZUK bir kayit ekle: uzunluk=10 de ama 3 bayt ver (yarim).
        {
            use std::io::Write;
            let mut f = OpenOptions::new().append(true).open(&p).unwrap();
            f.write_all(&10u32.to_be_bytes()).unwrap();
            f.write_all(&[1u8, 2, 3]).unwrap(); // 10 yerine 3 bayt -> yarim
        }
        let loaded = load_vertices(&p).expect("load");
        // Saglam ilk kayit korunur, yarim son kayit atlanir.
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0], vec![7u8, 7, 7]);
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn load_bytes_missing_is_none() {
        let p = temp_path("bytes-missing");
        assert!(load_bytes(&p).expect("load").is_none());
    }

    #[test]
    fn save_then_load_bytes_roundtrip() {
        let p = temp_path("bytes-roundtrip");
        let key = [42u8; 33]; // 1 algo bayti + 32 seed gibi
        save_bytes(&p, &key).expect("save");
        let loaded = load_bytes(&p).expect("load").expect("var");
        assert_eq!(loaded, key.to_vec());
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn save_overwrites_not_appends() {
        let p = temp_path("bytes-overwrite");
        save_bytes(&p, &[1u8, 2, 3]).expect("first");
        save_bytes(&p, &[9u8, 9]).expect("second");
        let loaded = load_bytes(&p).expect("load").expect("var");
        assert_eq!(
            loaded,
            vec![9u8, 9],
            "save_bytes uzerine yazar (append degil)"
        );
        let _ = std::fs::remove_file(&p);
    }
}
