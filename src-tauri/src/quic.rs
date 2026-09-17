use aes::cipher::generic_array::GenericArray;
use aes::cipher::{BlockEncrypt, KeyInit as AesKeyInit};
use aes::Aes128;
use aes_gcm::aead::{Aead, Payload};
use aes_gcm::{Aes128Gcm, KeyInit as GcmKeyInit};
use hmac::{Hmac, Mac};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

const QUIC_SALT: [u8; 20] = [
    0x38, 0x76, 0x2c, 0xf7, 0xf5, 0x59, 0x34, 0xb3, 0x4d, 0x17,
    0x9a, 0xe6, 0xa4, 0xc8, 0x0c, 0xad, 0xcc, 0xbb, 0x7f, 0x0a,
];

fn hmac_sha256(key: &[u8], data: &[u8]) -> [u8; 32] {
    let mut mac = <HmacSha256 as Mac>::new_from_slice(key).expect("hmac key");
    mac.update(data);
    let result = mac.finalize().into_bytes();
    let mut out = [0u8; 32];
    out.copy_from_slice(&result);
    out
}

fn derive_secret(prk: &[u8], length: usize, label: &str, context: &[u8]) -> Vec<u8> {
    let label_full = format!("tls13 {}", label);
    let mut info = Vec::new();
    info.extend_from_slice(&(length as u16).to_be_bytes());
    info.push(label_full.len() as u8);
    info.extend_from_slice(label_full.as_bytes());
    info.push(context.len() as u8);
    info.extend_from_slice(context);
    info.push(0x01);
    let mac = hmac_sha256(prk, &info);
    mac[..length].to_vec()
}

fn aes_gcm_encrypt(key: &[u8], nonce: &[u8], aad: &[u8], plaintext: &[u8]) -> Vec<u8> {
    let cipher = <Aes128Gcm as GcmKeyInit>::new_from_slice(key).expect("aes-gcm key");
    let payload = Payload { msg: plaintext, aad };
    cipher.encrypt(nonce.into(), payload).expect("aes-gcm encrypt")
}

fn aes_ecb_block(key: &[u8], data: &[u8]) -> [u8; 16] {
    let cipher = <Aes128 as AesKeyInit>::new_from_slice(key).expect("aes key");
    let mut block = GenericArray::clone_from_slice(&data[..16]);
    cipher.encrypt_block(&mut block);
    let mut out = [0u8; 16];
    out.copy_from_slice(&block);
    out
}

fn varint(value: u64) -> Vec<u8> {
    if value < 0x40 {
        vec![value as u8]
    } else if value < 0x4000 {
        let mut out = (value as u16).to_be_bytes().to_vec();
        out[0] |= 0x40;
        out
    } else if value < 0x40000000 {
        let mut out = (value as u32).to_be_bytes().to_vec();
        out[0] |= 0x80;
        out
    } else {
        let mut out = value.to_be_bytes().to_vec();
        out[0] |= 0xc0;
        out
    }
}

fn varint_length(value: u64) -> usize {
    if value < 0x40 {
        1
    } else if value < 0x4000 {
        2
    } else if value < 0x40000000 {
        4
    } else {
        8
    }
}

fn str16(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(2 + data.len());
    out.extend_from_slice(&(data.len() as u16).to_be_bytes());
    out.extend_from_slice(data);
    out
}

fn tls_client_hello_sni(sni: &str) -> Vec<u8> {
    let sni_bytes = sni.as_bytes();

    let mut ext_data = Vec::new();
    let list_len = (sni_bytes.len() + 3) as u16;
    ext_data.extend_from_slice(&list_len.to_be_bytes());
    ext_data.push(0);
    ext_data.extend_from_slice(&(sni_bytes.len() as u16).to_be_bytes());
    ext_data.extend_from_slice(sni_bytes);

    let mut sni_ext = Vec::new();
    sni_ext.extend_from_slice(&0u16.to_be_bytes());
    sni_ext.extend_from_slice(&(ext_data.len() as u16).to_be_bytes());
    sni_ext.extend_from_slice(&ext_data);

    let mut random = [0u8; 32];
    getrandom::getrandom(&mut random).expect("random");

    let mut body = Vec::new();
    body.push(0x03);
    body.push(0x03);
    body.extend_from_slice(&random);
    body.extend_from_slice(&[0, 0, 0, 0]);
    body.extend_from_slice(&str16(&sni_ext));

    let mut payload = Vec::new();
    payload.push(0x01);
    let len = body.len() as u32;
    payload.push((len >> 16) as u8);
    payload.push((len >> 8) as u8);
    payload.push(len as u8);
    payload.extend_from_slice(&body);
    payload
}

fn crypto_frame(data: &[u8], offset: u64) -> Vec<u8> {
    let mut out = Vec::new();
    out.push(0x06);
    out.extend_from_slice(&varint(offset));
    out.extend_from_slice(&varint(data.len() as u64));
    out.extend_from_slice(data);
    out
}

fn client_hello_to_frames(ch: &[u8]) -> Vec<u8> {
    let mut p2s = 38usize;
    while p2s < ch.len() && ch[p2s] == 0 {
        p2s += 1;
    }
    let mut payload = Vec::new();
    payload.extend_from_slice(&crypto_frame(&ch[0..1], 0));
    payload.extend_from_slice(&crypto_frame(&ch[p2s..], p2s as u64));
    payload
}

fn quic_initial(dcid: &[u8], scid: &[u8], token: &[u8], pkn: &[u8], payload: &[u8]) -> Vec<u8> {
    let tag_len = 16usize;
    let base_header_len = 8 + dcid.len() + scid.len() + token.len() + pkn.len();

    let mut padding = 0usize;
    if pkn.len() + payload.len() + padding + tag_len < 20 {
        padding = 20 - pkn.len() - payload.len() - tag_len;
    }
    let _ = (base_header_len, varint_length(0), padding);

    let mut header = Vec::new();
    header.push(0xc0 | ((pkn.len() as u8) - 1));
    header.extend_from_slice(&[0, 0, 0, 1]);
    header.push(dcid.len() as u8);
    header.extend_from_slice(dcid);
    header.push(scid.len() as u8);
    header.extend_from_slice(scid);
    header.push(token.len() as u8);
    header.extend_from_slice(token);
    header.extend_from_slice(&varint((pkn.len() + payload.len() + padding + tag_len) as u64));
    header.extend_from_slice(pkn);

    let init_secret = hmac_sha256(&QUIC_SALT, dcid);
    let client_secret = derive_secret(&init_secret, 32, "client in", &[]);
    let quic_key = derive_secret(&client_secret, 16, "quic key", &[]);
    let mut quic_iv = derive_secret(&client_secret, 12, "quic iv", &[]);
    let quic_hp = derive_secret(&client_secret, 16, "quic hp", &[]);

    let iv_start = 12 - pkn.len();
    for (i, b) in pkn.iter().enumerate() {
        quic_iv[iv_start + i] ^= b;
    }

    let mut padded = payload.to_vec();
    padded.extend(std::iter::repeat(0u8).take(padding));

    let encrypted = aes_gcm_encrypt(&quic_key, &quic_iv, &header, &padded);

    let sample_start = 4 - pkn.len();
    let sample_end = sample_start + 16;
    let mut mask = aes_ecb_block(&quic_hp, &encrypted[sample_start..sample_end]);
    mask[0] &= 0x0f;

    header[0] ^= mask[0];
    let pn_start = header.len() - pkn.len();
    for i in 0..pkn.len() {
        header[pn_start + i] ^= mask[1 + i];
    }

    let mut out = header;
    out.extend_from_slice(&encrypted);
    out
}

fn to_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push(HEX[(b >> 4) as usize] as char);
        s.push(HEX[(b & 0x0f) as usize] as char);
    }
    s
}

fn validate_domain(s: &str) -> Result<(), String> {
    if s.is_empty() {
        return Err("домен пустой".into());
    }
    if s.len() > 253 {
        return Err("домен длиннее 253 символов".into());
    }
    if !s.contains('.') {
        return Err("домен должен содержать точку (например ozon.ru)".into());
    }
    if s.starts_with('.') || s.ends_with('.') || s.contains("..") {
        return Err("некорректное расположение точек".into());
    }

    for label in s.split('.') {
        if label.is_empty() {
            return Err("пустая метка домена".into());
        }
        if label.len() > 63 {
            return Err("метка домена длиннее 63 символов".into());
        }
        if label.starts_with('-') || label.ends_with('-') {
            return Err("метка не может начинаться или заканчиваться дефисом".into());
        }
        for ch in label.chars() {
            let ok = ch.is_ascii_lowercase()
                || ch.is_ascii_uppercase()
                || ch.is_ascii_digit()
                || ch == '-';
            if !ok {
                return Err(format!(
                    "недопустимый символ '{}'. Только латиница, цифры и дефис",
                    ch
                ));
            }
        }
    }

    Ok(())
}

pub fn generate_i1(domain: &str) -> Result<String, String> {
    let lowered = domain.trim().to_lowercase();
    let sni = lowered.as_str();
    validate_domain(sni)?;

    let mut dcid = [0u8; 1];
    getrandom::getrandom(&mut dcid).map_err(|e| e.to_string())?;

    let ch = tls_client_hello_sni(sni);
    let payload = client_hello_to_frames(&ch);
    let packet = quic_initial(&dcid, &[], &[], &[0], &payload);

    Ok(format!("I1 = <b 0x{}>", to_hex(&packet)))
}