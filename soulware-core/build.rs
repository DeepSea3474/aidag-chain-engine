// İşlem izinde gösterilen yazılım sürümü: derleme anındaki git commit'i (+ "-degisik" kaydedilmemiş değişiklik varsa).
fn main() {
    let git = |a: &[&str]| std::process::Command::new("git").args(a).output().ok()
        .filter(|o| o.status.success()).map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string());
    let sha = git(&["rev-parse", "--short=12", "HEAD"]).unwrap_or_else(|| "bilinmiyor".into());
    let degisik = git(&["status", "--porcelain", "--", "."]).map(|s| !s.is_empty()).unwrap_or(false);
    println!("cargo:rustc-env=SOULWARE_GIT_SHA={sha}{}", if degisik { "-degisik" } else { "" });
    // Commit değişince (kaynak değişmese de) sürüm damgası yenilensin: HEAD ve dal referansı izlenir.
    if let Some(gd) = git(&["rev-parse", "--git-dir"]) {
        println!("cargo:rerun-if-changed={gd}/HEAD");
    }
    if let (Some(cd), Some(dal)) = (git(&["rev-parse", "--git-common-dir"]), git(&["symbolic-ref", "-q", "HEAD"])) {
        println!("cargo:rerun-if-changed={cd}/{dal}");
        println!("cargo:rerun-if-changed={cd}/packed-refs");
    }
    println!("cargo:rerun-if-changed=src");
    println!("cargo:rerun-if-changed=build.rs");
}
