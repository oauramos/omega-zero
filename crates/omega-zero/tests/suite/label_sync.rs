use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use omega_store::ps1::disc_label;
use omega_store::{Cart, Platform};
use omega_zero::json::Json;
use omega_zero::label_sync::{
    crc32, fetch_width, game_url, label_regions, lookups, media_url, pick_label, pick_media,
    read_reply, regions, sync, title, wants, Account, Curl, Found, Http, LabelNews, Lookup, Media,
    Quota, Reply, Sync, SyncFail, Tally, Want, Who,
};

#[test]
fn json_reads_what_screenscraper_sends_and_forgives_a_stray_backslash() {
    let j = Json::parse(
        r#"{"response": {"jeu": {"id": "3767", "noms": [{"text": "Café 🎮"}],
            "synopsis": "Samus\'s ship \n lands", "ok": true, "none": null, "n": -1.5e2}}}"#,
    )
    .expect("did not parse");
    let jeu = j.get("response").and_then(|r| r.get("jeu")).unwrap();
    assert_eq!(jeu.get("id").and_then(Json::count), Some(3767));
    assert_eq!(
        jeu.get("noms").unwrap().list()[0]
            .get("text")
            .and_then(Json::text),
        Some("Café 🎮")
    );
    assert_eq!(
        jeu.get("synopsis").and_then(Json::text),
        Some("Samus\\'s ship \n lands"),
        "a backslash JSON has no escape for is kept"
    );
    assert_eq!(jeu.get("ok"), Some(&Json::Bool(true)));
    assert_eq!(jeu.get("none"), Some(&Json::Null));
    assert_eq!(jeu.get("n"), Some(&Json::Number(-150.0)));
    assert!(Json::parse(r#"{"a": 1"#).is_none(), "an unclosed object");
    assert!(Json::parse(r#"{"a": 1} x"#).is_none(), "trailing text");
}

#[test]
fn a_cart_is_known_by_its_crc32() {
    assert_eq!(crc32(b"123456789"), 0xcbf4_3926);
    assert_eq!(crc32(b""), 0);
}

#[test]
fn a_label_is_taken_from_the_roms_own_region_first() {
    assert_eq!(
        regions("Metroid Fusion (USA)", "AMTE"),
        ["us", "wor", "eu", "jp"]
    );
    assert_eq!(
        regions("Golden Sun (USA, Europe)", "AGSE"),
        ["us", "eu", "wor", "jp"]
    );
    assert_eq!(
        regions(
            "Kirby & The Amazing Mirror (Europe) (En,Fr,De,Es,It)",
            "B8KP"
        ),
        ["eu", "wor", "us", "jp"],
        "language tags are not regions"
    );
    assert_eq!(
        regions("Mother 3 v1.2", "A3UJ"),
        ["jp", "wor", "us", "eu"],
        "a hack's name has no tags, but its game code does"
    );
    assert_eq!(regions("Tetris", ""), ["wor", "us", "eu", "jp"]);
}

#[test]
fn a_lookup_carries_both_credentials_and_the_rom_escaped() {
    let account = Account {
        user: "middle baws".into(),
        password: "p&ss=word".into(),
    };
    let url = game_url(
        ("devid", "devpw"),
        &account,
        &Lookup {
            system: 12,
            kind: "rom",
            crc: Some(0xabc),
            file_name: "Mario & Luigi (Europe).gba".into(),
            size: 16_777_216,
            serial: None,
        },
    );
    assert!(url.starts_with("https://api.screenscraper.fr/api2/jeuInfos.php?devid=devid&"));
    for part in [
        "devpassword=devpw",
        "softname=omega-zero",
        "output=json",
        "ssid=middle%20baws",
        "sspassword=p%26ss%3Dword",
        "crc=00000ABC",
        "systemeid=12",
        "romtype=rom",
        "romnom=Mario%20%26%20Luigi%20%28Europe%29.gba",
        "romtaille=16777216",
    ] {
        assert!(url.contains(part), "{part} is missing from {url}");
    }
    assert!(media_url("https://x/mediaJeu.php?a=1", Platform::Gba)
        .ends_with("&maxwidth=399&outputformat=png"));
}

#[test]
fn a_scan_is_fetched_half_as_wide_again_as_its_widest_face() {
    for (platform, width) in [
        (Platform::Gba, 399),
        (Platform::Gb, 330),
        (Platform::Gbc, 330),
        (Platform::Snes, 420),
        (Platform::Ps1, 432),
    ] {
        assert_eq!(fetch_width(platform), width, "{platform:?}");
        let widest = omega_ui::face_labels(platform).iter().map(|f| f.0).max();
        assert!(
            widest.is_some_and(|w| w < width),
            "{platform:?} covers its faces"
        );
    }
}

const GAME: &str = r#"{"header": {"success": "true"}, "response": {
    "ssuser": {"id": "middlebaws", "maxrequestsperday": "20000", "requeststoday": "47"},
    "jeu": {"id": "3767", "medias": [
        {"type": "box-2D", "parent": "jeu", "region": "us", "url": "https://ss/box-us", "format": "png"},
        {"type": "support-texture", "parent": "jeu", "region": "jp", "url": "https://ss/tex-jp", "format": "png"},
        {"type": "support-texture", "parent": "jeu", "region": "us", "url": "https://ss/tex-us", "format": "png"},
        {"type": "wheel", "parent": "jeu", "region": "wor", "url": "https://ss/wheel", "format": "png"}
    ]}}}"#;

#[test]
fn a_reply_names_the_game_its_media_and_the_lookups_left_today() {
    let (reply, quota) = read_reply(200, GAME.as_bytes());
    let Reply::Game(Found { medias, disc }) = reply else {
        panic!("{reply:?}");
    };
    assert_eq!(medias.len(), 4);
    assert_eq!(disc, None, "the reply names no disc");
    assert_eq!(
        quota,
        Some(Quota {
            left: 19_953,
            per_day: 20_000
        })
    );
    let picked = pick_label(&medias, &regions("Metroid Fusion (USA)", "AMTE")).unwrap();
    assert_eq!(picked.url, "https://ss/tex-us");
    let picked = pick_label(&medias, &["eu", "wor"]).unwrap();
    assert_eq!(picked.url, "https://ss/tex-jp", "any region beats none");
    let boxes: Vec<Media> = medias
        .into_iter()
        .filter(|m| m.kind != "support-texture")
        .collect();
    assert_eq!(pick_label(&boxes, &["us"]), None, "a box is not a label");
}

#[test]
fn a_refusal_says_whose_login_was_wrong() {
    let dev = "Erreur de login : Vérifier vos identifiants développeur !";
    assert_eq!(
        read_reply(403, dev.as_bytes()).0,
        Reply::Refused(Who::Program)
    );
    assert_eq!(
        read_reply(200, dev.as_bytes()).0,
        Reply::Refused(Who::Program),
        "a refusal can come back as a 200"
    );
    let player = "Erreur de login : Vérifier vos identifiants !";
    assert_eq!(
        read_reply(403, player.as_bytes()).0,
        Reply::Refused(Who::Player)
    );
    assert_eq!(
        read_reply(404, b"Erreur : Rom non trouvee").0,
        Reply::NotFound
    );
    assert_eq!(read_reply(431, b"").0, Reply::NotFound);
    assert_eq!(read_reply(429, b"").0, Reply::Busy);
    assert_eq!(read_reply(430, b"").0, Reply::Spent);
    for down in [401, 423, 426] {
        assert_eq!(read_reply(down, b"").0, Reply::Down);
    }
    assert_eq!(read_reply(200, br#"{"response": {}}"#).0, Reply::NotFound);
    assert_eq!(read_reply(200, b"<html>").0, Reply::Other(200));
}

#[test]
fn an_account_is_kept_in_config_and_can_be_forgotten() {
    let root = tempfile::tempdir().unwrap();
    assert_eq!(Account::load(root.path()), None);
    let account = Account {
        user: "middlebaws".into(),
        password: "pa=ss word".into(),
    };
    account.save(root.path()).unwrap();
    assert_eq!(Account::load(root.path()), Some(account));
    Account::forget(root.path());
    assert_eq!(Account::load(root.path()), None);
}

#[test]
fn a_title_drops_the_tags() {
    assert_eq!(title("Metroid Fusion (USA)"), "Metroid Fusion");
    assert_eq!(title("Mother 3 v1.2"), "Mother 3 v1.2");
}

fn png(w: u32, h: u32) -> Vec<u8> {
    let mut out = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut out, w, h);
        enc.set_color(png::ColorType::Rgb);
        enc.set_depth(png::BitDepth::Eight);
        let mut wr = enc.write_header().unwrap();
        wr.write_image_data(&vec![0x80; (w * h * 3) as usize])
            .unwrap();
    }
    out
}

/// Answers each GET from a queue, and remembers what it was asked.
struct Server {
    replies: VecDeque<Option<(u16, Vec<u8>)>>,
    asked: Vec<String>,
}

impl Server {
    fn new(replies: Vec<Option<(u16, Vec<u8>)>>) -> Server {
        Server {
            replies: replies.into(),
            asked: Vec::new(),
        }
    }
}

impl Http for Server {
    fn get(&mut self, url: &str) -> Option<(u16, Vec<u8>)> {
        self.asked.push(url.to_string());
        self.replies.pop_front().expect("asked more than expected")
    }
}

fn want(root: &Path, stem: &str) -> Want {
    let rom = root.join("Games/GBA").join(format!("{stem}.gba"));
    std::fs::create_dir_all(rom.parent().unwrap()).unwrap();
    std::fs::write(&rom, stem.as_bytes()).unwrap();
    Want {
        platform: Platform::Gba,
        stem: stem.into(),
        rom,
        code: String::new(),
        disc: 0,
        discs: 1,
    }
}

fn run(root: &Path, wants: &[Want], server: &mut Server) -> (Vec<LabelNews>, Vec<Duration>) {
    let account = Account {
        user: "u".into(),
        password: "p".into(),
    };
    let job = Sync {
        key: ("id", "pw"),
        account: &account,
        root,
        wants,
    };
    let (mut news, mut pauses) = (Vec::new(), Vec::new());
    sync(
        &job,
        server,
        &AtomicBool::new(false),
        &mut |d| pauses.push(d),
        &mut |n| news.push(n),
    );
    (news, pauses)
}

fn ok(body: &str) -> Option<(u16, Vec<u8>)> {
    Some((200, body.as_bytes().to_vec()))
}

#[test]
fn a_found_label_is_saved_where_the_shelf_looks_and_the_rest_are_counted() {
    let root = tempfile::tempdir().unwrap();
    let wants = [
        want(root.path(), "Metroid Fusion (USA)"),
        want(root.path(), "Box Only (USA)"),
        want(root.path(), "Mother 3 v1.2"),
    ];
    let bare = r#"{"response": {"jeu": {"medias": [
        {"type": "box-2D", "region": "us", "url": "https://ss/box"}]}}}"#;
    let mut server = Server::new(vec![
        ok(GAME),
        Some((200, png(600, 310))),
        ok(bare),
        Some((404, b"Erreur : Rom/Iso/Dossier non trouvee !".to_vec())),
    ]);
    let (news, _) = run(root.path(), &wants, &mut server);
    let label = root.path().join("Labels/GBA/Metroid Fusion (USA).png");
    assert_eq!(std::fs::read(&label).unwrap(), png(600, 310));
    assert!(server.asked[0].contains(&format!("crc={:08X}", crc32(b"Metroid Fusion (USA)"))));
    assert!(server.asked[0].contains("romtaille=20"));
    assert_eq!(
        server.asked[1],
        "https://ss/tex-us&maxwidth=399&outputformat=png"
    );
    assert!(news.contains(&LabelNews::Saved {
        platform: Platform::Gba,
        stem: "Metroid Fusion (USA)".into()
    }));
    assert!(news.contains(&LabelNews::Working {
        done: 2,
        total: 3,
        title: "Mother 3 v1.2".into()
    }));
    assert_eq!(
        news.last(),
        Some(&LabelNews::Finished(Tally {
            saved: 1,
            bare: 1,
            unknown: 1
        }))
    );
    assert!(!root.path().join("Labels/GBA/Box Only (USA).png").exists());
}

#[test]
fn a_scan_that_is_not_a_picture_is_not_saved() {
    let root = tempfile::tempdir().unwrap();
    let wants = [want(root.path(), "Metroid Fusion (USA)")];
    let mut server = Server::new(vec![ok(GAME), ok("NOMEDIA")]);
    let (news, _) = run(root.path(), &wants, &mut server);
    assert!(!root
        .path()
        .join("Labels/GBA/Metroid Fusion (USA).png")
        .exists());
    assert_eq!(
        news.last(),
        Some(&LabelNews::Finished(Tally {
            bare: 1,
            ..Tally::default()
        }))
    );
}

#[test]
fn a_busy_server_is_asked_once_more_after_a_pause() {
    let root = tempfile::tempdir().unwrap();
    let wants = [want(root.path(), "Metroid Fusion (USA)")];
    let mut server = Server::new(vec![
        Some((429, Vec::new())),
        ok(GAME),
        Some((200, png(4, 2))),
    ]);
    let (news, pauses) = run(root.path(), &wants, &mut server);
    assert_eq!(pauses, [Duration::from_secs(2)]);
    assert!(matches!(news.last(), Some(LabelNews::Finished(t)) if t.saved == 1));
}

#[test]
fn a_refused_login_or_a_spent_quota_ends_the_sync_at_once() {
    for (reply, fail) in [
        (
            (403, "Erreur de login : Vérifier vos identifiants !"),
            SyncFail::Refused(Who::Player),
        ),
        ((430, ""), SyncFail::Spent),
        ((423, ""), SyncFail::Down),
    ] {
        let root = tempfile::tempdir().unwrap();
        let wants = [want(root.path(), "A (USA)"), want(root.path(), "B (USA)")];
        let mut server = Server::new(vec![Some((reply.0, reply.1.as_bytes().to_vec()))]);
        let (news, _) = run(root.path(), &wants, &mut server);
        assert_eq!(news.last(), Some(&LabelNews::Failed(fail)));
        assert_eq!(
            server.asked.len(),
            1,
            "{fail:?}: the next cart was still asked about"
        );
    }
}

#[test]
fn no_connection_ends_the_sync_as_offline() {
    let root = tempfile::tempdir().unwrap();
    let wants = [want(root.path(), "A (USA)")];
    let mut server = Server::new(vec![None]);
    let (news, _) = run(root.path(), &wants, &mut server);
    assert_eq!(news.last(), Some(&LabelNews::Failed(SyncFail::Offline)));
}

#[test]
fn the_quota_is_passed_on_as_it_comes() {
    let root = tempfile::tempdir().unwrap();
    let wants = [want(root.path(), "A (USA)")];
    let mut server = Server::new(vec![ok(GAME), Some((200, png(2, 1)))]);
    let (news, _) = run(root.path(), &wants, &mut server);
    assert!(news.contains(&LabelNews::Quota(Quota {
        left: 19_953,
        per_day: 20_000
    })));
}

#[test]
fn the_key_build_rs_masks_reads_back() {
    // build.rs masks "devid" byte by byte with 0x5a, 0x79, 0x98, ...
    let masked: String = "devid"
        .bytes()
        .enumerate()
        .map(|(i, b)| {
            format!(
                "{:02x}",
                b ^ 0x5a_u8.wrapping_add((i as u8).wrapping_mul(31))
            )
        })
        .collect();
    assert_eq!(
        omega_zero::label_sync::unmask(&masked).as_deref(),
        Some("devid")
    );
    assert_eq!(omega_zero::label_sync::unmask("zz"), None);
    assert_eq!(
        omega_zero::label_sync::dev_key(),
        None,
        "this test build has a key"
    );
}

fn cart(platform: Platform, stem: &str, rom: PathBuf, label: Option<PathBuf>) -> Cart {
    Cart {
        platform,
        stem: stem.into(),
        rom,
        label,
        title: String::new(),
        code: String::new(),
        shell: None,
    }
}

fn touch(path: &Path, bytes: &[u8]) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, bytes).unwrap();
}

/// A PlayStation game on three discs under a playlist, its cue sheets standing in for discs.
fn three_discs(root: &Path) -> (Cart, Vec<PathBuf>) {
    let dir = root.join("Games/PS1");
    let discs: Vec<PathBuf> = (1..=3)
        .map(|n| dir.join(format!("Final Fantasy VII (USA) (Disc {n}).cue")))
        .collect();
    for d in &discs {
        touch(d, b"FILE \"x.bin\" BINARY\n");
    }
    let m3u = dir.join("Final Fantasy VII (USA).m3u");
    touch(
        &m3u,
        b"Final Fantasy VII (USA) (Disc 1).cue\nFinal Fantasy VII (USA) (Disc 2).cue\n\
          Final Fantasy VII (USA) (Disc 3).cue\n",
    );
    let mut game = cart(Platform::Ps1, "Final Fantasy VII (USA)", m3u, None);
    game.code = "SCUS-94163".into();
    (game, discs)
}

#[test]
fn every_disc_with_no_label_of_its_own_is_wanted() {
    let root = tempfile::tempdir().unwrap();
    let root = root.path();
    let (game, discs) = three_discs(root);
    touch(&disc_label(root, &discs[1]), b"png");
    let lone = root.join("Games/PS1/Vagrant Story (USA).cue");
    touch(&lone, b"cue");
    let labelled_lone = cart(
        Platform::Ps1,
        "Vagrant Story (USA)",
        lone.clone(),
        Some(root.join("Labels/PS1/Vagrant Story (USA).png")),
    );
    touch(&root.join("Labels/PS1/Vagrant Story (USA).png"), b"png");
    let gba = root.join("Games/GBA/Fusion.gba");
    let carts = [
        cart(Platform::Gba, "Fusion", gba.clone(), None),
        cart(
            Platform::Gba,
            "Zero",
            root.join("Games/GBA/Zero.gba"),
            Some(root.join("Labels/GBA/Zero.png")),
        ),
        game.clone(),
        labelled_lone,
    ];

    let wanted = wants(Some(root), &carts);

    let seen: Vec<(&str, PathBuf, usize, usize, &str)> = wanted
        .iter()
        .map(|w| {
            (
                w.stem.as_str(),
                w.rom.clone(),
                w.disc,
                w.discs,
                w.code.as_str(),
            )
        })
        .collect();
    assert_eq!(
        seen,
        [
            ("Fusion", gba, 0, 1, ""),
            (
                "Final Fantasy VII (USA)",
                discs[0].clone(),
                0,
                3,
                "SCUS-94163"
            ),
            ("Final Fantasy VII (USA)", discs[2].clone(), 2, 3, ""),
        ],
        "a labelled disc, cart or lone disc was wanted, or an unlabelled one was not"
    );
    assert_eq!(wanted[2].title(), "Final Fantasy VII, disc 3");
    assert_eq!(wanted[0].title(), "Fusion");

    // the first disc has a label of its own, but its game has none to stand on the shelf with
    touch(&disc_label(root, &discs[0]), b"png");
    let again = wants(Some(root), std::slice::from_ref(&game));
    assert_eq!(again.iter().map(|w| w.disc).collect::<Vec<_>>(), [0, 2]);
    let mut shelved = game.clone();
    shelved.label = Some(disc_label(root, &discs[0]));
    let again = wants(Some(root), std::slice::from_ref(&shelved));
    assert_eq!(again.iter().map(|w| w.disc).collect::<Vec<_>>(), [2]);
}

#[test]
fn a_discs_label_is_kept_under_its_own_name_and_the_first_is_its_games_too() {
    let root = Path::new("/sd");
    let card = tempfile::tempdir().unwrap();
    let (game, discs) = three_discs(card.path());
    let want = |disc: usize| Want {
        rom: discs[disc].clone(),
        disc,
        discs: 3,
        ..Want::cart(&game)
    };
    let one = |name: &str| root.join("Labels/PS1").join(name);
    assert_eq!(
        want(0).label_paths(root),
        [
            one("Final Fantasy VII (USA) (Disc 1).png"),
            one("Final Fantasy VII (USA).png")
        ]
    );
    assert_eq!(
        want(1).label_paths(root),
        [one("Final Fantasy VII (USA) (Disc 2).png")]
    );
    let lone = cart(
        Platform::Ps1,
        "Vagrant Story (USA)",
        root.join("Games/PS1/Vagrant Story (USA)/Vagrant Story (USA).chd"),
        None,
    );
    assert_eq!(
        Want::cart(&lone).label_paths(root),
        [one("Vagrant Story (USA).png")]
    );
    let snes = cart(
        Platform::Snes,
        "Super Metroid (USA)",
        root.join("Games/SNES/Super Metroid (USA).sfc"),
        None,
    );
    assert_eq!(
        Want::cart(&snes).label_paths(root),
        [root.join("Labels/SNES/Super Metroid (USA).png")]
    );
}

#[test]
fn a_disc_is_asked_about_by_its_cue_sheet_then_its_serial_then_its_name_never_read_whole() {
    let root = tempfile::tempdir().unwrap();
    let (game, discs) = three_discs(root.path());
    let sheet = std::fs::read(&discs[0]).unwrap();
    let first = Want {
        rom: discs[0].clone(),
        discs: 3,
        ..Want::cart(&game)
    };
    let named = Lookup {
        system: 57,
        kind: "iso",
        crc: None,
        file_name: "Final Fantasy VII (USA) (Disc 1).cue".into(),
        size: sheet.len() as u64,
        serial: None,
    };
    assert_eq!(
        lookups(&first),
        [
            Lookup {
                crc: Some(crc32(&sheet)),
                ..named.clone()
            },
            Lookup {
                serial: Some("SCUS-94163".into()),
                ..named.clone()
            },
            named.clone(),
        ]
    );
    let urls: Vec<String> = lookups(&first)
        .iter()
        .map(|l| game_url(("id", "pw"), &Account::default(), l))
        .collect();
    for url in &urls {
        for part in [
            "systemeid=57",
            "romtype=iso",
            "romnom=Final%20Fantasy%20VII%20%28USA%29%20%28Disc%201%29.cue",
            &format!("romtaille={}", sheet.len()),
        ] {
            assert!(url.contains(part), "{part} is missing from {url}");
        }
    }
    assert!(urls[0].contains(&format!("crc={:08X}", crc32(&sheet))));
    assert!(!urls[0].contains("serialnum"));
    assert!(urls[1].contains("serialnum=SCUS-94163") && !urls[1].contains("crc="));
    assert!(!urls[2].contains("crc=") && !urls[2].contains("serialnum"));

    // a later disc's serial is read when it is looked up; these stand-ins have none
    let third = Want {
        rom: discs[2].clone(),
        code: String::new(),
        disc: 2,
        discs: 3,
        ..Want::cart(&game)
    };
    let asked = lookups(&third);
    assert_eq!(asked.len(), 2);
    assert!(asked.iter().all(|l| l.serial.is_none()));

    // a compressed image is asked about by its name and size alone
    let chd = root.path().join("Games/PS1/Ape Escape (USA).chd");
    touch(&chd, &vec![0u8; 200 * 1024]);
    let lone = cart(Platform::Ps1, "Ape Escape (USA)", chd, None);
    assert_eq!(
        lookups(&Want::cart(&lone)),
        [Lookup {
            file_name: "Ape Escape (USA).chd".into(),
            size: 200 * 1024,
            ..named.clone()
        }]
    );

    let gone = Want {
        rom: root.path().join("Games/PS1/Gone.chd"),
        ..Want::cart(&game)
    };
    assert!(lookups(&gone).is_empty());
}

#[test]
fn a_snes_cart_is_asked_about_by_its_crc_and_a_copier_dump_by_its_roms_as_well() {
    let root = tempfile::tempdir().unwrap();
    let rom = root.path().join("Games/SNES/Super Metroid (USA).sfc");
    touch(&rom, b"123456789");
    let snes = cart(Platform::Snes, "Super Metroid (USA)", rom, None);
    let asked = lookups(&Want::cart(&snes));
    assert_eq!(asked.len(), 1);
    assert_eq!(asked[0].system, 4);
    assert_eq!(asked[0].kind, "rom");
    assert_eq!(asked[0].crc, Some(0xcbf4_3926));
    let url = game_url(("id", "pw"), &Account::default(), &asked[0]);
    for part in [
        "systemeid=4",
        "romtype=rom",
        "crc=CBF43926",
        "romtaille=9",
        "romnom=Super%20Metroid%20%28USA%29.sfc",
    ] {
        assert!(url.contains(part), "{part} is missing from {url}");
    }
    assert!(!url.contains("serialnum"));

    let smc = root.path().join("Games/SNES/Super Metroid (U) [!].smc");
    let mut dump = vec![0xaa_u8; 512];
    dump.extend(vec![0x55_u8; 1024]);
    touch(&smc, &dump);
    let copier = cart(Platform::Snes, "Super Metroid (U) [!]", smc, None);
    let asked = lookups(&Want::cart(&copier));
    assert_eq!(asked.len(), 2);
    assert_eq!(
        (asked[0].crc, asked[0].size),
        (Some(crc32(&dump)), 1536),
        "not the dump as it is first"
    );
    assert_eq!(
        (asked[1].crc, asked[1].size),
        (Some(crc32(&dump[512..])), 1024),
        "not the ROM after the copier's header"
    );

    // the second way is asked only when ScreenScraper does not know the first
    let wants = [Want::cart(&copier)];
    let mut server = Server::new(vec![
        Some((404, b"Erreur : Rom/Iso/Dossier non trouvee !".to_vec())),
        ok(GAME),
        Some((200, png(4, 2))),
    ]);
    let (news, _) = run(root.path(), &wants, &mut server);
    assert!(server.asked[1].contains(&format!("crc={:08X}", crc32(&dump[512..]))));
    assert!(root
        .path()
        .join("Labels/SNES/Super Metroid (U) [!].png")
        .is_file());
    assert!(matches!(news.last(), Some(LabelNews::Finished(t)) if t.saved == 1));
}

const AMERICAN: [&str; 3] = ["us", "ca", "br"];
const EUROPEAN: [&str; 9] = ["eu", "uk", "fr", "de", "sp", "it", "nl", "se", "au"];

fn joined(parts: &[&[&'static str]]) -> Vec<&'static str> {
    parts.iter().flat_map(|p| p.iter().copied()).collect()
}

#[test]
fn a_snes_label_suits_the_shell_its_cart_is_drawn_in() {
    let snes = |stem: &str| {
        Want::cart(&cart(
            Platform::Snes,
            stem,
            PathBuf::from(format!("/nowhere/{stem}.sfc")),
            None,
        ))
    };
    let american_shell = joined(&[&AMERICAN, &["wor"], &EUROPEAN, &["jp"]]);
    assert_eq!(label_regions(&snes("Super Metroid (USA)")), american_shell);
    assert_eq!(
        label_regions(&snes("Super Metroid (USA, Europe)")),
        american_shell,
        "a European label went before an American one on an American shell"
    );
    assert_eq!(
        label_regions(&snes("Super Metroid (Japan)")),
        joined(&[&["jp"], &EUROPEAN, &["wor"], &AMERICAN])
    );
    assert_eq!(
        label_regions(&snes("Super Metroid (Europe)")),
        joined(&[&EUROPEAN, &["jp", "wor"], &AMERICAN]),
        "ScreenScraper keeps the SNES's European labels by country"
    );
    assert_eq!(
        label_regions(&snes("Super Metroid (Germany)"))[..3],
        ["de", "eu", "uk"]
    );
    assert_eq!(
        label_regions(&snes("Super Metroid (Japan, USA)")),
        joined(&[&["jp"], &EUROPEAN, &["wor"], &AMERICAN]),
        "a Super Famicom shell takes an American label last"
    );
}

#[test]
fn a_discs_label_is_taken_from_its_serials_region_after_its_name() {
    let disc = |stem: &str, serial: &str| Want {
        code: serial.into(),
        ..Want::cart(&cart(
            Platform::Ps1,
            stem,
            PathBuf::from(format!("/nowhere/{stem}.cue")),
            None,
        ))
    };
    assert_eq!(
        label_regions(&disc("Tekken 3", "SCES-01237")),
        joined(&[&EUROPEAN, &["wor", "us", "jp"]])
    );
    assert_eq!(
        label_regions(&disc("Tekken 3 (Japan)", "SCES-01237"))[..2],
        ["jp", "eu"]
    );
    assert_eq!(
        label_regions(&disc("Tekken 3", "SLUS-00402")),
        ["us", "wor", "eu", "jp"]
    );
    assert_eq!(
        label_regions(&disc("Tekken 3", "")),
        ["wor", "us", "eu", "jp"]
    );
}

/// A PlayStation game's reply whose discs' faces are numbered, naming the disc asked about
/// when `disc` says.
fn disc_game(disc: Option<u32>) -> String {
    let rom = disc
        .map(|n| format!(r#""rom": {{"romnumsupport": "{n}", "romtotalsupport": "3"}},"#))
        .unwrap_or_default();
    format!(
        r#"{{"response": {{"jeu": {{"id": "19249", {rom} "medias": [
        {{"type": "support-texture", "parent": "jeu", "region": "us", "support": "1", "url": "https://ss/tex-us-1"}},
        {{"type": "support-texture", "parent": "jeu", "region": "jp", "support": "2", "url": "https://ss/tex-jp-2"}},
        {{"type": "support-texture", "parent": "jeu", "region": "us", "support": "2", "url": "https://ss/tex-us-2"}},
        {{"type": "support-2D", "parent": "jeu", "region": "us", "support": "1", "url": "https://ss/2d-us-1"}},
        {{"type": "support-2D", "parent": "jeu", "region": "us", "support": "3", "url": "https://ss/2d-us-3"}},
        {{"type": "support-texture", "parent": "systeme", "region": "us", "support": "3", "url": "https://ss/console"}},
        {{"type": "box-2D", "parent": "jeu", "region": "us", "url": "https://ss/box-us"}}
    ]}}}}}}"#
    )
}

fn found(body: &str) -> Found {
    match read_reply(200, body.as_bytes()).0 {
        Reply::Game(found) => found,
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_disc_takes_the_scan_of_its_own_face() {
    let root = tempfile::tempdir().unwrap();
    let (game, discs) = three_discs(root.path());
    let want = |disc: usize| Want {
        rom: discs[disc].clone(),
        disc,
        discs: 3,
        ..Want::cart(&game)
    };
    let url = |f: &Found, disc: usize| pick_media(f, &want(disc)).map(|m| m.url.clone());
    let unnamed = found(&disc_game(None));
    assert_eq!(url(&unnamed, 0).as_deref(), Some("https://ss/tex-us-1"));
    assert_eq!(url(&unnamed, 1).as_deref(), Some("https://ss/tex-us-2"));
    assert_eq!(
        url(&unnamed, 2).as_deref(),
        Some("https://ss/2d-us-3"),
        "with no scan of the third disc, the face ScreenScraper draws of it is next best"
    );
    assert_eq!(
        url(&found(&disc_game(Some(2))), 0).as_deref(),
        Some("https://ss/tex-us-2"),
        "the disc the reply says the file is wins over the playlist's order"
    );
    assert!(
        unnamed.medias.iter().all(|m| m.url != "https://ss/console"),
        "a medium that is not the game's own was kept"
    );

    let unnumbered = found(
        r#"{"response": {"jeu": {"medias": [
        {"type": "support-texture", "region": "jp", "url": "https://ss/tex-jp"}]}}}"#,
    );
    assert_eq!(
        url(&unnumbered, 0).as_deref(),
        Some("https://ss/tex-jp"),
        "an unnumbered face is the first disc's"
    );
    assert_eq!(
        url(&unnumbered, 1),
        None,
        "a later disc took the first's face"
    );

    let lone = cart(
        Platform::Ps1,
        "Vagrant Story (USA)",
        root.path().join("Games/PS1/Vagrant Story (USA).cue"),
        None,
    );
    assert_eq!(
        pick_media(&unnumbered, &Want::cart(&lone)).map(|m| m.url.as_str()),
        Some("https://ss/tex-jp")
    );
}

#[test]
fn each_disc_saves_its_face_and_the_first_disc_labels_its_game_too() {
    let root = tempfile::tempdir().unwrap();
    let (game, discs) = three_discs(root.path());
    let wants = [
        Want {
            rom: discs[0].clone(),
            discs: 3,
            ..Want::cart(&game)
        },
        Want {
            rom: discs[1].clone(),
            code: String::new(),
            disc: 1,
            discs: 3,
            ..Want::cart(&game)
        },
    ];
    let not_found = Some((404, b"Erreur : Rom/Iso/Dossier non trouvee !".to_vec()));
    let mut server = Server::new(vec![
        ok(&disc_game(Some(1))),
        Some((200, png(300, 300))),
        // the second disc's cue sheet is not one ScreenScraper knows, but its name is
        not_found,
        ok(&disc_game(Some(2))),
        Some((200, png(301, 301))),
    ]);
    let (news, _) = run(root.path(), &wants, &mut server);
    assert_eq!(
        server.asked[1],
        "https://ss/tex-us-1&maxwidth=432&outputformat=png"
    );
    assert!(server.asked[2].contains("crc="));
    assert!(!server.asked[3].contains("crc=") && !server.asked[3].contains("serialnum"));
    assert_eq!(
        server.asked[4],
        "https://ss/tex-us-2&maxwidth=432&outputformat=png"
    );
    let labels = root.path().join("Labels/PS1");
    let read = |name: &str| std::fs::read(labels.join(name)).unwrap();
    assert_eq!(read("Final Fantasy VII (USA) (Disc 1).png"), png(300, 300));
    assert_eq!(read("Final Fantasy VII (USA).png"), png(300, 300));
    assert_eq!(read("Final Fantasy VII (USA) (Disc 2).png"), png(301, 301));
    assert!(!labels.join("Final Fantasy VII (USA) (Disc 3).png").exists());
    let saved = LabelNews::Saved {
        platform: Platform::Ps1,
        stem: "Final Fantasy VII (USA)".into(),
    };
    assert_eq!(news.iter().filter(|n| **n == saved).count(), 2);
    assert!(news.contains(&LabelNews::Working {
        done: 1,
        total: 2,
        title: "Final Fantasy VII, disc 2".into()
    }));
    assert_eq!(
        news.last(),
        Some(&LabelNews::Finished(Tally {
            saved: 2,
            ..Tally::default()
        }))
    );
}

#[test]
fn a_disc_no_way_of_asking_finds_is_not_on_screenscraper() {
    let root = tempfile::tempdir().unwrap();
    let (game, discs) = three_discs(root.path());
    let wants = [Want {
        rom: discs[0].clone(),
        discs: 3,
        ..Want::cart(&game)
    }];
    let not_found = || Some((404, b"Erreur : Rom/Iso/Dossier non trouvee !".to_vec()));
    let mut server = Server::new(vec![not_found(), not_found(), not_found()]);
    let (news, _) = run(root.path(), &wants, &mut server);
    assert_eq!(server.asked.len(), 3, "cue sheet, serial, name");
    assert_eq!(
        news.last(),
        Some(&LabelNews::Finished(Tally {
            unknown: 1,
            ..Tally::default()
        }))
    );
}

/// Serves one request on a port of its own, answering it 200 with `body`, and hands back the
/// port and the request's first line.
fn serve_once(body: &'static [u8]) -> (u16, std::thread::JoinHandle<String>) {
    use std::io::{BufRead, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = std::thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        let mut reader = std::io::BufReader::new(stream.try_clone().unwrap());
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        let mut rest = String::new();
        while reader.read_line(&mut rest).is_ok_and(|n| n > 2) {
            rest.clear();
        }
        let mut stream = stream;
        write!(
            stream,
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        )
        .unwrap();
        stream.write_all(body).unwrap();
        line
    });
    (port, server)
}

#[test]
fn curl_fetches_a_disc_s_media_whose_name_carries_its_number_in_brackets() {
    if std::process::Command::new("curl")
        .arg("--version")
        .output()
        .is_err()
    {
        eprintln!("no curl on this host, skipping");
        return;
    }
    let (port, server) = serve_once(b"PNG");
    let url = format!(
        "http://127.0.0.1:{port}/api2/mediaJeu.php?jeuid=19252&media=support-texture(us)[1]&maxwidth=432"
    );
    assert_eq!(
        Curl.get(&url),
        Some((200, b"PNG".to_vec())),
        "curl read the disc's number as a range and asked nothing"
    );
    assert!(server.join().unwrap().contains("support-texture(us)[1]"));
}
