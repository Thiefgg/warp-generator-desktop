use crate::quic;

const SNI_POOL: &[&str] = &[
    "ozon.ru",
    "rutub.ru",
    "vk.com",
    "ya.ru",
    "mail.ru",
    "avito.ru",
    "wildberries.ru",
    "kinopoisk.ru",
    "t.me",
    "sberbank.ru",
];

pub struct Stealth {
    pub profile: &'static str,
    pub sni: String,
    pub init_spread: u32,
    pub jitter: u32,
    pub junk: u32,
    pub junk_min: u32,
    pub junk_max: u32,
    pub s1: u32,
    pub s2: u32,
    pub h1: u32,
    pub h2: u32,
    pub h3: u32,
    pub h4: u32,
}

pub fn rand_u32() -> u32 {
    let mut b = [0u8; 4];
    getrandom::getrandom(&mut b).expect("random");
    u32::from_le_bytes(b)
}

pub fn pick_sni() -> String {
    SNI_POOL[rand_u32() as usize % SNI_POOL.len()].to_string()
}

pub fn pick_profile(name: &str) -> Stealth {
    match name {
        "paranoid" => {
            let (a, b) = (20 + rand_u32() % 40, 120 + rand_u32() % 140);
            Stealth {
                profile: "paranoid",
                sni: pick_sni(),
                init_spread: rand_u32() % 9,
                jitter: 80 + rand_u32() % 60,
                junk: 10 + rand_u32() % 6,
                junk_min: a,
                junk_max: b,
                s1: 8 + rand_u32() % 12,
                s2: 8 + rand_u32() % 12,
                h1: rand_u32() % 5 + 1,
                h2: rand_u32() % 9 + 1,
                h3: rand_u32() % 13 + 1,
                h4: rand_u32() % 17 + 1,
            }
        }
        "light" => {
            let (a, b) = (10 + rand_u32() % 10, 40 + rand_u32() % 30);
            Stealth {
                profile: "light",
                sni: pick_sni(),
                init_spread: rand_u32() % 3,
                jitter: 20 + rand_u32() % 20,
                junk: rand_u32() % 3 + 1,
                junk_min: a,
                junk_max: b,
                s1: 0,
                s2: 0,
                h1: 1,
                h2: 2,
                h3: 3,
                h4: 4,
            }
        }
        _ => {
            let (a, b) = (40 + rand_u32() % 30, 70 + rand_u32() % 50);
            Stealth {
                profile: "standard",
                sni: pick_sni(),
                init_spread: rand_u32() % 5,
                jitter: 35 + rand_u32() % 30,
                junk: 4 + rand_u32() % 4,
                junk_min: a,
                junk_max: b,
                s1: 4 + rand_u32() % 6,
                s2: 4 + rand_u32() % 6,
                h1: rand_u32() % 4 + 1,
                h2: rand_u32() % 7 + 1,
                h3: rand_u32() % 11 + 1,
                h4: rand_u32() % 15 + 1,
            }
        }
    }
}

pub fn build(stealth: &Stealth) -> Result<String, String> {
    let i1 = quic::generate_i1(&stealth.sni)?;
    Ok(format!(
        "\n[Stealth]\nProfile = {}\nSni = {}\nInitSpread = {}\nTimingJitter = {}\nJunkRatio = {}\nJunkCount = {}\nJunkMin = {}\nJunkMax = {}\nS1 = {}\nS2 = {}\nH1 = {}\nH2 = {}\nH3 = {}\nH4 = {}\n{}\n",
        stealth.profile,
        stealth.sni,
        stealth.init_spread,
        stealth.jitter,
        format!("{:.2}", stealth.junk as f32 / 100.0),
        stealth.junk,
        stealth.junk_min,
        stealth.junk_max,
        stealth.s1,
        stealth.s2,
        stealth.h1,
        stealth.h2,
        stealth.h3,
        stealth.h4,
        i1
    ))
}