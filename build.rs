fn push_prop(buf: &mut Vec<u8>, key_le: &[u8; 4], data: &[u8]) {
    // Vendor '8BIM' in Windows PiPL little-endian order = "MIB8"
    buf.extend_from_slice(b"MIB8");
    buf.extend_from_slice(key_le);
    buf.extend_from_slice(&0u32.to_le_bytes()); // property id = 0
    let padded_len = (data.len() + 3) & !3;
    buf.extend_from_slice(&(padded_len as u32).to_le_bytes());
    buf.extend_from_slice(data);
    for _ in data.len()..padded_len {
        buf.push(0);
    }
}

fn pascal_bytes(s: &str) -> Vec<u8> {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(1 + bytes.len());
    out.push(bytes.len() as u8);
    out.extend_from_slice(bytes);
    out
}

fn cstr_bytes(s: &str) -> Vec<u8> {
    let mut out = s.as_bytes().to_vec();
    out.push(0);
    out
}

fn build_aegx_pipl() -> Vec<u8> {
    let mut buf = Vec::new();
    buf.extend_from_slice(&1u16.to_le_bytes()); // reserved = 1
    buf.extend_from_slice(&0u32.to_le_bytes()); // PiPL version = 0
    buf.extend_from_slice(&6u32.to_le_bytes()); // 6 properties

    // 1. Kind = 'AEgx' (Windows LE = b"xgEA") — True startup AEGP General Plugin!
    push_prop(&mut buf, b"dnik", b"xgEA");
    // 2. Name = "FXSearch"
    push_prop(&mut buf, b"eman", &pascal_bytes("FXSearch"));
    // 3. Category = "General Plugin"
    push_prop(&mut buf, b"gtac", &pascal_bytes("General Plugin"));
    // 4. Version = 1.0 (0x00010000)
    push_prop(&mut buf, b"srev", &[0x00, 0x00, 0x01, 0x00]);
    // 5. CodeWin64X86 = "EntryPointFunc"
    push_prop(&mut buf, b"4668", &cstr_bytes("EntryPointFunc"));
    // 6. AE_Reserved_Info = 0
    push_prop(&mut buf, b"LFea", &[0x00, 0x00, 0x00, 0x00]);

    buf
}

fn main() {
    println!("cargo:rerun-if-changed=src/host_script.jsx");
    println!("cargo:rerun-if-changed=build.rs");

    let pipl_bytes = build_aegx_pipl();
    let escaped: String = pipl_bytes
        .iter()
        .map(|b| format!("\\x{:02x}", b))
        .collect();

    let mut res = winres::WindowsResource::new();
    res.append_rc_content(&format!(
        "16000 PiPL DISCARDABLE BEGIN \"{}\" END\n",
        escaped
    ));
    res.compile().expect("Failed to compile AEgx PiPL resource");
}
