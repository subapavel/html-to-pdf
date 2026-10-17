//! What `Renderer` logs about images (F32a). One test only: the logger is global to the process.

use std::sync::Mutex;

use htmltopdf::{RenderOptions, Renderer};

static LOG: Mutex<Vec<String>> = Mutex::new(Vec::new());

struct Capture;

impl log::Log for Capture {
    fn enabled(&self, m: &log::Metadata<'_>) -> bool {
        m.level() <= log::Level::Warn
    }
    fn log(&self, r: &log::Record<'_>) {
        if r.level() <= log::Level::Warn && r.target().starts_with("htmltopdf") {
            LOG.lock().unwrap().push(r.args().to_string());
        }
    }
    fn flush(&self) {}
}

const JPEG: &[u8] = include_bytes!("data/gradient64.jpg");

/// `JPEG` with three fill bytes in front of the first marker after APP0 (T.81 §B.1.1.2 allows them).
fn jpeg_with_fill_bytes() -> Vec<u8> {
    let app0_end = 2 + 2 + usize::from(u16::from_be_bytes([JPEG[4], JPEG[5]]));
    let mut out = JPEG[..app0_end].to_vec();
    out.extend_from_slice(&[0xFF; 3]);
    out.extend_from_slice(&JPEG[app0_end..]);
    out
}

fn take() -> Vec<String> {
    std::mem::take(&mut *LOG.lock().unwrap())
}

#[test]
fn warnings_name_the_images_that_are_missing_or_damaged_and_only_those() {
    log::set_boxed_logger(Box::new(Capture)).unwrap();
    log::set_max_level(log::LevelFilter::Warn);
    let r = Renderer::builder()
        .image("ok.jpg", JPEG.to_vec())
        .image("fill.jpg", jpeg_with_fill_bytes())
        .image("cut.jpg", JPEG[..JPEG.len() * 3 / 4].to_vec())
        .image("bad.jpg", JPEG[..10].to_vec())
        // Registered and broken, never referenced: decoded by nobody, mentioned by nobody.
        .image("unused.png", b"garbage".to_vec())
        .build();
    let render = |body: &str| {
        r.render(
            &format!("<!doctype html><html><body>{body}</body></html>"),
            &RenderOptions::default(),
        )
        .expect("warnings only")
    };

    render(r#"<img src="ok.jpg">"#);
    assert_eq!(take(), Vec::<String>::new(), "an intact image is silent");

    render(r#"<img src="fill.jpg">"#);
    assert_eq!(
        take(),
        Vec::<String>::new(),
        "a JPEG with fill bytes is valid and silent"
    );

    render(r#"<img src="cut.jpg">"#);
    let log = take();
    assert_eq!(log.len(), 1, "{log:?}");
    assert!(
        log[0].contains("cut.jpg") && log[0].contains("embedded although damaged"),
        "{log:?}"
    );

    render(r#"<img src="bad.jpg"><img src="nope.jpg">"#);
    let log = take();
    assert_eq!(log.len(), 2, "{log:?}");
    assert!(
        log[0].contains("bad.jpg") && log[0].contains("cannot be decoded"),
        "{log:?}"
    );
    assert!(
        log[1].contains("nope.jpg") && log[1].contains("not a bundled image"),
        "{log:?}"
    );

    // The same renderer, the same document: the verdicts are cached, the messages are not.
    render(r#"<img src="cut.jpg">"#);
    assert_eq!(take().len(), 1);
}
