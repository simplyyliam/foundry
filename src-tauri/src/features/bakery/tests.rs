use std::collections::BTreeMap;
use std::fs;
use std::io::Cursor;
use std::path::Path;
use std::sync::Arc;

use image::{DynamicImage, GenericImageView, ImageFormat, Rgb, RgbImage};

use super::*;
use crate::testing::TestRepo;

const P: &str = "project-1";

// ---------- fixtures ----------

struct Env {
    repo: TestRepo,
    repos: Arc<RepoIndex>,
    store: Arc<BakeryStore>,
}

impl Env {
    fn new() -> Self {
        let repo = TestRepo::new();
        let repos = Arc::new(RepoIndex::default());
        repos.replace_all([(P, repo.path())]);
        Self {
            store: Arc::new(BakeryStore::new(Arc::clone(&repos))),
            repos,
            repo,
        }
    }

    fn path(&self, rel: &str) -> PathBuf {
        self.repo.path().join(rel)
    }

    fn manifest(&self) -> Vec<u8> {
        fs::read(self.path(MANIFEST_FILE)).unwrap()
    }

    fn layout(&self) -> serde_json::Value {
        serde_json::from_slice(&fs::read(self.path(LAYOUT_FILE)).unwrap()).unwrap()
    }

    fn add_png(&self, w: u32, h: u32) -> BakeryItem {
        let result = self
            .store
            .add_from_bytes(P, "paste.png", &png(w, h))
            .unwrap();
        item(&result.bakery, &result.added[0])
    }

    fn file_bytes(&self, file: &str) -> Vec<u8> {
        fs::read(self.path(&format!("{IMAGE_DIR}/{file}"))).unwrap()
    }
}

fn item(state: &BakeryState, id: &str) -> BakeryItem {
    state.items.iter().find(|i| i.id == id).unwrap().clone()
}

fn encode(img: DynamicImage, format: ImageFormat) -> Vec<u8> {
    let mut out = Vec::new();
    img.write_to(&mut Cursor::new(&mut out), format).unwrap();
    out
}

/// Left half red, right half blue.
fn two_tone(w: u32, h: u32) -> RgbImage {
    RgbImage::from_fn(w, h, |x, _| {
        if x < w / 2 {
            Rgb([255, 0, 0])
        } else {
            Rgb([0, 0, 255])
        }
    })
}

fn png(w: u32, h: u32) -> Vec<u8> {
    encode(DynamicImage::ImageRgb8(two_tone(w, h)), ImageFormat::Png)
}

fn webp(w: u32, h: u32) -> Vec<u8> {
    encode(DynamicImage::ImageRgb8(two_tone(w, h)), ImageFormat::WebP)
}

/// A JPEG with an EXIF APP1 segment saying "rotate 90° clockwise to display" (orientation 6),
/// plus a camera-ish make tag, so both orientation and stripping can be checked.
fn jpeg_rotated(w: u32, h: u32) -> Vec<u8> {
    let plain = encode(DynamicImage::ImageRgb8(two_tone(w, h)), ImageFormat::Jpeg);
    #[rustfmt::skip]
    let tiff: Vec<u8> = vec![
        b'M', b'M', 0x00, 0x2A, 0x00, 0x00, 0x00, 0x08, // big-endian header, IFD at 8
        0x00, 0x02,                                     // 2 entries
        0x01, 0x0F, 0x00, 0x02, 0x00, 0x00, 0x00, 0x04, // Make, ASCII, 4 bytes inline
        b'C', b'a', b'm', 0x00,
        0x01, 0x12, 0x00, 0x03, 0x00, 0x00, 0x00, 0x01, // Orientation, SHORT, 1
        0x00, 0x06, 0x00, 0x00,                         // = 6
        0x00, 0x00, 0x00, 0x00,                         // no next IFD
    ];
    let mut app1 = b"Exif\0\0".to_vec();
    app1.extend_from_slice(&tiff);
    let len = u16::try_from(app1.len() + 2).unwrap();
    let mut out = vec![0xFF, 0xD8, 0xFF, 0xE1];
    out.extend_from_slice(&len.to_be_bytes());
    out.extend_from_slice(&app1);
    out.extend_from_slice(&plain[2..]); // everything after the original SOI
    out
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack.windows(needle.len()).any(|w| w == needle)
}

fn snapshot(dir: &Path) -> BTreeMap<String, Vec<u8>> {
    fn walk(root: &Path, dir: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
        for entry in fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                walk(root, &path, out);
            } else {
                let rel = path
                    .strip_prefix(root)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/");
                out.insert(rel, fs::read(&path).unwrap());
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(dir, dir, &mut out);
    out
}

fn label(env: &Env, id: &str, label: &str, take: &str) -> BakeryState {
    env.store
        .update(
            P,
            id,
            BakeryPatch {
                label: Some(label.into()),
                take: Some(take.into()),
                ..Default::default()
            },
        )
        .unwrap()
}

// ---------- R-5-1 ----------

#[test]
fn large_images_are_downscaled_to_1600_and_small_ones_are_not_upscaled() {
    let env = Env::new();
    let wide = env.add_png(3200, 1000);
    let tall = env
        .store
        .add_from_bytes(P, "tall.webp", &webp(900, 2400))
        .unwrap();
    let tall = item(&tall.bakery, &tall.added[0]);
    let small = env.add_png(300, 200);

    let dims = |file: &str| {
        image::load_from_memory(&env.file_bytes(file))
            .unwrap()
            .dimensions()
    };
    assert_eq!(dims(&wide.file), (1600, 500));
    assert_eq!(dims(&tall.file), (600, 1600));
    assert_eq!(dims(&small.file), (300, 200));
    assert!(wide.file.ends_with(".png"));
    assert!(
        tall.file.ends_with(".png"),
        "WebP becomes PNG: {}",
        tall.file
    );
    assert_eq!(
        image::guess_format(&env.file_bytes(&tall.file)).unwrap(),
        ImageFormat::Png
    );
}

// ---------- R-5-2 ----------

#[test]
fn exif_rotated_photos_come_out_upright_without_metadata() {
    let env = Env::new();
    let input = jpeg_rotated(80, 40);
    assert!(contains(&input, b"Exif"), "fixture carries EXIF");

    let result = env.store.add_from_bytes(P, "photo.jpg", &input).unwrap();
    let file = item(&result.bakery, &result.added[0]).file;
    assert!(file.ends_with(".jpg"), "JPEG stays JPEG: {file}");
    let bytes = env.file_bytes(&file);

    let img = image::load_from_memory(&bytes).unwrap().to_rgb8();
    assert_eq!(img.dimensions(), (40, 80), "rotated to portrait");
    // Rotating 90° clockwise puts the red left half on top.
    let top = img.get_pixel(20, 10);
    let bottom = img.get_pixel(20, 70);
    assert!(top[0] > 200 && top[2] < 60, "top should be red: {top:?}");
    assert!(
        bottom[2] > 200 && bottom[0] < 60,
        "bottom should be blue: {bottom:?}"
    );

    assert!(!contains(&bytes, b"Exif"), "EXIF stripped");
    assert!(!contains(&bytes, b"Cam\0"), "camera make stripped");
    assert!(!contains(&bytes, &[0xFF, 0xE1]), "no APP1 segment left");
}

// ---------- R-5-3 ----------

#[test]
fn non_images_oversized_files_and_traversal_names_are_rejected() {
    let env = Env::new();
    let code = |result: Result<BakeryAddResult, AppError>| result.unwrap_err().code;

    // Not an image, whatever the name says; and a real but unsupported format.
    assert_eq!(
        code(env.store.add_from_bytes(P, "notes.png", b"just some text")),
        ErrorCode::UnsupportedImage
    );
    assert_eq!(
        code(
            env.store
                .add_from_bytes(P, "anim.gif", b"GIF89a\x01\x00\x01\x00\x00\x00\x00;")
        ),
        ErrorCode::UnsupportedImage
    );
    // Over 25 MB, as bytes and as a dropped file (checked before reading it).
    let big = vec![0u8; usize::try_from(MAX_INPUT_BYTES).unwrap() + 1];
    assert_eq!(
        code(env.store.add_from_bytes(P, "big.png", &big)),
        ErrorCode::FileTooLarge
    );
    let outside = TestRepo::new();
    let huge = outside.path().join("huge.png");
    fs::File::create(&huge)
        .unwrap()
        .set_len(MAX_INPUT_BYTES + 1)
        .unwrap();
    assert_eq!(
        code(
            env.store
                .add_from_paths(P, &[huge.to_string_lossy().into_owned()], None)
        ),
        ErrorCode::FileTooLarge
    );
    // Traversal in a pasted name, and in a dropped path.
    for name in [
        "../evil.png",
        "..\\evil.png",
        "docs/x.png",
        "/abs.png",
        "C:\\x.png",
        "..",
    ] {
        let err = env.store.add_from_bytes(P, name, &png(10, 10)).unwrap_err();
        assert!(
            matches!(err.code, ErrorCode::PathEscape | ErrorCode::InvalidPath),
            "{name}: {err}"
        );
    }
    let sneaky = format!("{}/../huge.png", outside.path().display());
    assert_eq!(
        code(env.store.add_from_paths(P, &[sneaky], None)),
        ErrorCode::InvalidPath
    );
    assert_eq!(
        code(env.store.add_from_paths(P, &["relative.png".into()], None)),
        ErrorCode::InvalidPath
    );

    // Nothing was written anywhere.
    assert!(!env.path(IMAGE_DIR).exists());
    assert!(!env.path(LAYOUT_FILE).exists());
}

#[test]
fn one_bad_file_in_a_drop_adds_nothing() {
    let env = Env::new();
    let outside = TestRepo::new();
    let good = outside.path().join("good.png");
    let bad = outside.path().join("bad.png");
    fs::write(&good, png(20, 20)).unwrap();
    fs::write(&bad, "not an image").unwrap();
    let paths = [good, bad].map(|p| p.to_string_lossy().into_owned());

    let err = env.store.add_from_paths(P, &paths, None).unwrap_err();

    assert_eq!(err.code, ErrorCode::UnsupportedImage);
    assert!(env.store.list(P).unwrap().items.is_empty());
}

#[test]
fn dropped_files_land_at_the_drop_point_and_cascade() {
    let env = Env::new();
    let outside = TestRepo::new();
    let paths: Vec<String> = (0..3)
        .map(|i| {
            let path = outside.path().join(format!("{i}.png"));
            fs::write(&path, png(400, 200)).unwrap();
            path.to_string_lossy().into_owned()
        })
        .collect();

    let result = env
        .store
        .add_from_paths(
            P,
            &paths,
            Some(DropPoint {
                x: Coord(100.0),
                y: Coord(50.0),
            }),
        )
        .unwrap();

    let items: Vec<BakeryItem> = result
        .added
        .iter()
        .map(|id| item(&result.bakery, id))
        .collect();
    assert_eq!((items[0].x.0, items[0].y.0), (100.0, 50.0));
    assert_eq!((items[1].x.0, items[1].y.0), (124.0, 74.0));
    assert_eq!((items[0].w.0, items[0].h.0), (240.0, 120.0), "aspect kept");
    assert!(items[0].z < items[1].z && items[1].z < items[2].z);
    let files: Vec<&str> = items.iter().map(|i| i.file.as_str()).collect();
    assert_eq!(files, ["01.png", "02.png", "03.png"]);
}

// ---------- R-5-4 ----------

#[test]
fn manifest_is_byte_identical_for_identical_state() {
    let build = || {
        let env = Env::new();
        let a = env.add_png(30, 30);
        let b = env.add_png(30, 30);
        env.add_png(30, 30);
        label(&env, &a.id, "Red door", "the colour\nand the  brass knob");
        env.store
            .update(
                P,
                &b.id,
                BakeryPatch {
                    tags: Some(vec![
                        "warm".into(),
                        " ".into(),
                        "warm".into(),
                        "wood".into(),
                    ]),
                    applies_to: Some(vec!["designer".into()]),
                    skip: Some("the people".into()),
                    ..Default::default()
                },
            )
            .unwrap();
        env.store
            .move_item(P, &a.id, Coord(500.0), Coord(10.0), None, None, Some(9))
            .unwrap();
        env
    };
    let one = build();
    let two = build();
    assert_eq!(one.manifest(), two.manifest());

    // Repeating an update that changes nothing doesn't change a byte either.
    let before = one.manifest();
    let a = one.store.list(P).unwrap().items[0].clone();
    label(&one, &a.id, "Red door", "the colour\nand the  brass knob");
    assert_eq!(one.manifest(), before);

    let text = String::from_utf8(before).unwrap();
    let expected = "\
## 01-red-door.png
- Label: Red door
- Take: the colour and the brass knob
- Skip: (none)
- Tags: (none)
- Applies to: (none)

## 02.png
- Label: (unlabeled)
- Take: (unlabeled)
- Skip: the people
- Tags: warm, wood
- Applies to: designer

## 03.png
- Label: (unlabeled)
- Take: (unlabeled)
- Skip: (none)
- Tags: (none)
- Applies to: (none)
";
    assert!(text.ends_with(expected), "{text}");
    assert!(text.starts_with("# Bakery manifest\n"));
    assert!(text.contains("Read this file first"));
    assert!(text.contains("Open an image only when its entry applies to your task"));
    assert!(text.contains("3 image(s), 2 unlabeled."));
}

#[test]
fn manifest_orders_by_number_whatever_the_layout_order() {
    let env = Env::new();
    for _ in 0..3 {
        env.add_png(10, 10);
    }
    let before = env.manifest();
    // Reverse the items in bakery.json by hand; the next manifest write must not differ.
    let mut layout = env.layout();
    layout["items"].as_array_mut().unwrap().reverse();
    fs::write(env.path(LAYOUT_FILE), layout.to_string()).unwrap();
    let id = env.store.list(P).unwrap().items[0].id.clone();
    env.store
        .update(
            P,
            &id,
            BakeryPatch {
                skip: Some(String::new()),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(env.manifest(), before);
}

// ---------- R-5-5 ----------

#[test]
fn bakery_move_leaves_the_manifest_byte_identical() {
    let env = Env::new();
    let a = env.add_png(40, 40);
    label(&env, &a.id, "Door", "shape");
    let before = env.manifest();
    let modified = fs::metadata(env.path(MANIFEST_FILE))
        .unwrap()
        .modified()
        .unwrap();

    let moved = env
        .store
        .move_item(
            P,
            &a.id,
            Coord(-12.5),
            Coord(300.0),
            Some(Coord(480.0)),
            Some(Coord(320.0)),
            Some(7),
        )
        .unwrap();

    assert_eq!(env.manifest(), before);
    assert_eq!(
        fs::metadata(env.path(MANIFEST_FILE))
            .unwrap()
            .modified()
            .unwrap(),
        modified,
        "manifest not even rewritten"
    );
    let moved = item(&moved, &a.id);
    assert_eq!(
        (moved.x.0, moved.y.0, moved.w.0, moved.h.0, moved.z),
        (-12.5, 300.0, 480.0, 320.0, 7)
    );
    assert_eq!(env.layout()["items"][0]["x"], -12.5, "layout saved");

    for (w, h) in [
        (Some(0.0), None),
        (None, Some(-1.0)),
        (Some(f64::NAN), None),
    ] {
        let (w, h) = (w.map(Coord), h.map(Coord));
        let err = env
            .store
            .move_item(P, &a.id, Coord(0.0), Coord(0.0), w, h, None)
            .unwrap_err();
        assert_eq!(err.code, ErrorCode::InvalidInput);
    }
}

// ---------- R-5-6 ----------

#[test]
fn bakery_remove_deletes_only_its_own_file_inside_docs_bakery() {
    let env = Env::new();
    let a = env.add_png(20, 20);
    let b = env.add_png(20, 20);
    fs::write(env.path("keep.txt"), "keep").unwrap();
    let mut before = snapshot(env.repo.path());

    let state = env.store.remove(P, &a.id).unwrap();

    assert_eq!(state.items.len(), 1);
    assert_eq!(state.items[0].id, b.id);
    let after = snapshot(env.repo.path());
    before.remove(&format!("{IMAGE_DIR}/{}", a.file)).unwrap();
    // Only the image went; layout and manifest changed; everything else is identical.
    for (path, bytes) in &before {
        if path != LAYOUT_FILE && path != MANIFEST_FILE {
            assert_eq!(after.get(path), Some(bytes), "{path} changed");
        }
    }
    assert_eq!(after.len(), before.len());
    assert!(!String::from_utf8(env.manifest()).unwrap().contains(&a.file));
}

#[test]
fn bakery_remove_refuses_a_layout_entry_pointing_outside_docs_bakery() {
    let env = Env::new();
    let a = env.add_png(20, 20);
    fs::create_dir_all(env.path("src")).unwrap();
    fs::write(env.path("src/main.rs"), "fn main() {}").unwrap();
    fs::write(env.path("docs/bakery/manifest.md.bak"), "x").unwrap();
    for evil in ["../../src/main.rs", "..\\..\\src\\main.rs", "manifest.md"] {
        let mut layout = env.layout();
        layout["items"][0]["file"] = evil.into();
        fs::write(env.path(LAYOUT_FILE), layout.to_string()).unwrap();
        let before = snapshot(env.repo.path());

        let err = env.store.remove(P, &a.id).unwrap_err();

        assert_eq!(err.code, ErrorCode::PathEscape, "{evil}");
        assert_eq!(snapshot(env.repo.path()), before, "{evil}: nothing changed");
    }
    assert!(env.path("src/main.rs").exists());
}

// ---------- R-5-7 ----------

#[test]
fn label_changes_rename_the_file_and_keep_layout_and_manifest_consistent() {
    let env = Env::new();
    let a = env.add_png(20, 20);
    let original = env.file_bytes(&a.file);
    assert_eq!(a.file, "01.png");

    let state = label(&env, &a.id, "Red Door!", "colour");
    let renamed = item(&state, &a.id);
    assert_eq!(renamed.file, "01-red-door.png");
    assert_eq!(env.file_bytes("01-red-door.png"), original);
    assert!(!env.path("docs/bakery/01.png").exists());
    assert_eq!(env.layout()["items"][0]["file"], "01-red-door.png");
    let manifest = String::from_utf8(env.manifest()).unwrap();
    assert!(manifest.contains("## 01-red-door.png\n"));
    assert!(!manifest.contains("## 01.png"));

    // Clearing the label goes back to NN.ext.
    let state = label(&env, &a.id, "  ", "colour");
    assert_eq!(item(&state, &a.id).file, "01.png");
    assert!(env.path("docs/bakery/01.png").exists());
}

#[test]
fn label_rename_on_a_name_collision_picks_a_free_name_and_stays_consistent() {
    let env = Env::new();
    let a = env.add_png(20, 20);
    let b = env.add_png(20, 20);
    // A stray file someone copied in holds the name the label wants.
    fs::write(env.path("docs/bakery/01-sky.png"), "stray").unwrap();

    let state = label(&env, &a.id, "Sky", "blue");

    let a_file = item(&state, &a.id).file;
    assert_eq!(a_file, "01-sky-2.png");
    assert_eq!(
        fs::read(env.path("docs/bakery/01-sky.png")).unwrap(),
        b"stray",
        "stray untouched"
    );
    assert!(env.path("docs/bakery/01-sky-2.png").exists());
    assert!(!env.path("docs/bakery/01.png").exists());
    assert_eq!(env.layout()["items"][0]["file"], "01-sky-2.png");
    assert!(String::from_utf8(env.manifest())
        .unwrap()
        .contains("## 01-sky-2.png\n"));

    // Same label on another image: its own NN keeps it distinct.
    let state = label(&env, &b.id, "Sky", "blue");
    assert_eq!(item(&state, &b.id).file, "02-sky.png");
    // A later notes-only edit keeps the collision name stable.
    let state = env
        .store
        .update(
            P,
            &a.id,
            BakeryPatch {
                skip: Some("clouds".into()),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(item(&state, &a.id).file, "01-sky-2.png");
    // Every layout file exists on disk.
    for it in &state.items {
        assert!(
            env.path(&format!("docs/bakery/{}", it.file)).exists(),
            "{}",
            it.file
        );
    }
}

#[test]
fn numbers_are_never_reused_after_a_remove() {
    let env = Env::new();
    let a = env.add_png(10, 10);
    env.add_png(10, 10);
    env.store.remove(P, &a.id).unwrap();
    let c = env.add_png(10, 10);
    assert_eq!(c.file, "03.png");

    // A stray file holding the next number is skipped, not replaced.
    fs::write(env.path("docs/bakery/04.png"), "stray").unwrap();
    let d = env.add_png(10, 10);
    assert_eq!(d.file, "05.png");
    assert_eq!(fs::read(env.path("docs/bakery/04.png")).unwrap(), b"stray");
}

// ---------- R-5-8 ----------

#[test]
fn ten_concurrent_adds_get_ten_distinct_numbers() {
    let env = Env::new();
    let image = png(64, 64);
    let handles: Vec<_> = (0..10)
        .map(|_| {
            let store = Arc::clone(&env.store);
            let image = image.clone();
            std::thread::spawn(move || store.add_from_bytes(P, "paste.png", &image).unwrap())
        })
        .collect();
    for handle in handles {
        handle.join().unwrap();
    }

    let state = env.store.list(P).unwrap();
    let mut numbers: Vec<u32> = state.items.iter().map(|i| file_number(&i.file)).collect();
    numbers.sort_unstable();
    assert_eq!(numbers, (1..=10).collect::<Vec<_>>());
    let on_disk = fs::read_dir(env.path(IMAGE_DIR))
        .unwrap()
        .filter(|e| e.as_ref().unwrap().file_name() != "manifest.md")
        .count();
    assert_eq!(on_disk, 10);
    assert_eq!(env.layout()["items"].as_array().unwrap().len(), 10);
}

// ---------- R-5-9 ----------

#[test]
fn unlabeled_count_is_correct_after_every_change() {
    let env = Env::new();
    let a = env.add_png(10, 10);
    let b = env.add_png(10, 10);
    let result = env.store.add_from_bytes(P, "c.png", &png(10, 10)).unwrap();
    assert_eq!(result.bakery.unlabeled_count, 3);
    let c = result.added[0].clone();

    // A label alone isn't enough; a Take note alone isn't either.
    assert_eq!(label(&env, &a.id, "Door", "").unlabeled_count, 3);
    assert_eq!(label(&env, &b.id, "", "shape").unlabeled_count, 3);
    assert_eq!(label(&env, &a.id, "Door", "colour").unlabeled_count, 2);
    assert_eq!(label(&env, &b.id, "Window", "shape").unlabeled_count, 1);
    let moved = env
        .store
        .move_item(P, &c, Coord(1.0), Coord(1.0), None, None, None)
        .unwrap();
    assert_eq!(moved.unlabeled_count, 1);
    assert_eq!(env.store.remove(P, &c).unwrap().unlabeled_count, 0);
    assert_eq!(env.store.list(P).unwrap().unlabeled_count, 0);
    assert!(String::from_utf8(env.manifest())
        .unwrap()
        .contains("2 image(s), 0 unlabeled."));
}

// ---------- R-5-10 ----------

#[test]
fn the_41st_image_is_refused() {
    let env = Env::new();
    let image = png(8, 8);
    for _ in 0..40 {
        env.store.add_from_bytes(P, "x.png", &image).unwrap();
    }
    let before = snapshot(env.repo.path());

    let err = env.store.add_from_bytes(P, "x.png", &image).unwrap_err();

    assert_eq!(err.code, ErrorCode::BakeryFull);
    assert_eq!(snapshot(env.repo.path()), before, "nothing written");
    assert_eq!(env.store.list(P).unwrap().max_images, DEFAULT_MAX_IMAGES);
}

#[test]
fn max_images_comes_from_bakery_json_and_a_batch_cannot_overflow_it() {
    let env = Env::new();
    env.add_png(8, 8);
    let mut layout = env.layout();
    layout["maxImages"] = 3.into();
    fs::write(env.path(LAYOUT_FILE), layout.to_string()).unwrap();
    let outside = TestRepo::new();
    let paths: Vec<String> = (0..3)
        .map(|i| {
            let path = outside.path().join(format!("{i}.png"));
            fs::write(&path, png(8, 8)).unwrap();
            path.to_string_lossy().into_owned()
        })
        .collect();

    let err = env.store.add_from_paths(P, &paths, None).unwrap_err();

    assert_eq!(err.code, ErrorCode::BakeryFull);
    assert_eq!(env.store.list(P).unwrap().items.len(), 1);
    env.store.add_from_paths(P, &paths[..2], None).unwrap();
    assert_eq!(env.store.list(P).unwrap().items.len(), 3);
}

// ---------- other ----------

#[test]
fn unknown_projects_and_items_are_reported() {
    let env = Env::new();
    assert_eq!(
        env.store.list("nope").unwrap_err().code,
        ErrorCode::NotFound
    );
    assert_eq!(
        env.store.remove(P, "missing").unwrap_err().code,
        ErrorCode::NotFound
    );
    assert_eq!(
        env.store
            .update(P, "missing", BakeryPatch::default())
            .unwrap_err()
            .code,
        ErrorCode::NotFound
    );
}

#[test]
fn an_empty_bakery_lists_without_writing_anything() {
    let env = Env::new();
    let state = env.store.list(P).unwrap();
    assert!(state.items.is_empty());
    assert_eq!(state.unlabeled_count, 0);
    assert_eq!(state.max_images, 40);
    assert_eq!(Path::new(&state.dir), env.repo.path().join("docs/bakery"));
    assert!(snapshot(env.repo.path()).is_empty());
}

#[test]
fn a_minimal_hand_written_bakery_json_loads_with_defaults() {
    let env = Env::new();
    fs::create_dir_all(env.path(".foundry")).unwrap();
    fs::create_dir_all(env.path(IMAGE_DIR)).unwrap();
    fs::write(env.path("docs/bakery/07-cat.png"), png(10, 10)).unwrap();
    fs::write(
        env.path(LAYOUT_FILE),
        r#"{ "items": [{ "id": "a", "file": "07-cat.png", "label": "Cat" }] }"#,
    )
    .unwrap();

    let state = env.store.list(P).unwrap();

    assert_eq!(state.items[0].label, "Cat");
    assert_eq!(state.items[0].take, "");
    assert_eq!(state.max_images, DEFAULT_MAX_IMAGES);
    assert_eq!(state.unlabeled_count, 1, "no Take note yet");
    assert_eq!(
        env.add_png(10, 10).file,
        "08.png",
        "numbering continues past 07"
    );
}

#[test]
fn a_corrupt_bakery_json_is_set_aside_not_lost() {
    let env = Env::new();
    fs::create_dir_all(env.path(".foundry")).unwrap();
    fs::write(env.path(LAYOUT_FILE), "{ not json").unwrap();

    assert!(env.store.list(P).unwrap().items.is_empty());

    let aside: Vec<_> = fs::read_dir(env.path(".foundry"))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|n| n.starts_with("bakery.json.corrupt-"))
        .collect();
    assert_eq!(aside.len(), 1);
    assert_eq!(
        fs::read_to_string(env.path(&format!(".foundry/{}", aside[0]))).unwrap(),
        "{ not json"
    );
}

#[test]
fn commands_round_trip_over_ipc() {
    use tauri::test::{mock_builder, mock_context, noop_assets, INVOKE_KEY};
    use tauri::{Manager, WebviewWindowBuilder};

    let env = Env::new();
    let app = crate::app_builder(mock_builder())
        .build(mock_context(noop_assets()))
        .unwrap();
    app.manage(Arc::clone(&env.store));
    let webview = WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .unwrap();
    let invoke = |cmd: &str, body: serde_json::Value| {
        tauri::test::get_ipc_response(
            &webview,
            tauri::webview::InvokeRequest {
                cmd: cmd.into(),
                callback: tauri::ipc::CallbackFn(0),
                error: tauri::ipc::CallbackFn(1),
                url: if cfg!(windows) {
                    "http://tauri.localhost"
                } else {
                    "tauri://localhost"
                }
                .parse()
                .unwrap(),
                body: tauri::ipc::InvokeBody::Json(body),
                headers: Default::default(),
                invoke_key: INVOKE_KEY.to_string(),
            },
        )
        .map(|body| body.deserialize::<serde_json::Value>().unwrap())
    };

    let added = invoke(
        "bakery_add_from_bytes",
        serde_json::json!({ "projectId": P, "name": "paste.png", "bytes": png(12, 12) }),
    )
    .unwrap();
    assert_eq!(added["bakery"]["unlabeledCount"], 1);
    let id = added["added"][0].as_str().unwrap().to_string();

    let outside = TestRepo::new();
    let dropped = outside.path().join("drop.png");
    fs::write(&dropped, png(12, 12)).unwrap();
    let from_paths = invoke(
        "bakery_add_from_paths",
        serde_json::json!({ "projectId": P, "paths": [dropped], "dropPoint": { "x": 5, "y": 6 } }),
    )
    .unwrap();
    assert_eq!(from_paths["bakery"]["items"][1]["x"], 5.0);

    let updated = invoke(
        "bakery_update",
        serde_json::json!({ "projectId": P, "id": id, "patch": {
            "label": "Hi", "take": "all", "skip": null, "tags": null, "appliesTo": ["designer"]
        } }),
    )
    .unwrap();
    assert_eq!(updated["items"][0]["file"], "01-hi.png");
    assert_eq!(updated["items"][0]["appliesTo"][0], "designer");

    let moved = invoke(
        "bakery_move",
        serde_json::json!({ "projectId": P, "id": id, "x": 1, "y": 2, "w": null, "h": null, "z": 3 }),
    )
    .unwrap();
    assert_eq!(moved["items"][0]["z"], 3);

    let listed = invoke("bakery_list", serde_json::json!({ "projectId": P })).unwrap();
    assert_eq!(listed["items"].as_array().unwrap().len(), 2);
    let removed = invoke(
        "bakery_remove",
        serde_json::json!({ "projectId": P, "id": id }),
    )
    .unwrap();
    assert_eq!(removed["items"].as_array().unwrap().len(), 1);

    let refused = invoke(
        "bakery_add_from_bytes",
        serde_json::json!({ "projectId": P, "name": "../x.png", "bytes": png(4, 4) }),
    )
    .unwrap_err();
    assert_eq!(refused["code"], "path_escape");
}

#[test]
fn asset_protocol_scope_allows_exactly_the_bakery_folder_once_used() {
    use tauri::test::{mock_builder, mock_context, noop_assets};
    use tauri::Manager;

    let env = Env::new();
    let app = crate::app_builder(mock_builder())
        .build(mock_context(noop_assets()))
        .unwrap();
    // The same wiring the real app uses, on this test's repo index.
    let store = crate::bakery_store(app.handle(), Arc::clone(&env.repos));
    let scope = app.asset_protocol_scope();
    // Built per component: mklink (used for the link below) rejects mixed separators.
    let bakery_dir = env.repo.path().join("docs").join("bakery");
    assert!(
        !scope.is_allowed(bakery_dir.join("01.png")),
        "nothing allowed up front"
    );

    store.list(P).unwrap();
    assert!(
        !scope.is_allowed(bakery_dir.join("01.png")),
        "no folder yet"
    );

    let added = store.add_from_bytes(P, "paste.png", &png(10, 10)).unwrap();
    let file = item(&added.bakery, &added.added[0]).file;
    // What the UI will do: convertFileSrc(`${dir}/${file}`).
    assert!(scope.is_allowed(Path::new(&added.bakery.dir).join(&file)));

    let patch = BakeryPatch {
        label: Some("Red door".into()),
        ..Default::default()
    };
    let renamed = store.update(P, &added.added[0], patch).unwrap();
    assert!(scope.is_allowed(Path::new(&renamed.dir).join("01-red-door.png")));

    // Outside docs/bakery stays closed: repo files, Foundry's own state, subfolders, links.
    fs::write(env.path("keep.txt"), "keep").unwrap();
    fs::create_dir_all(bakery_dir.join("sub")).unwrap();
    fs::write(bakery_dir.join("sub").join("x.png"), png(4, 4)).unwrap();
    let outside = TestRepo::new();
    fs::write(outside.path().join("secret.png"), png(4, 4)).unwrap();
    crate::testing::link_dir(outside.path(), &bakery_dir.join("linked"));
    for denied in [
        env.path("keep.txt"),
        env.path(LAYOUT_FILE),
        bakery_dir.join("sub").join("x.png"),
        bakery_dir.join("linked").join("secret.png"),
        outside.path().join("secret.png"),
        bakery_dir.join("..").join("..").join("keep.txt"),
    ] {
        assert!(
            !scope.is_allowed(&denied),
            "{} should be denied",
            denied.display()
        );
    }
}
