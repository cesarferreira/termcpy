use base64::{Engine, engine::general_purpose::STANDARD};
use flate2::read::ZlibDecoder;
use std::io::Read;
use termcpy::{
    frame::Frame,
    geometry::Viewport,
    graphics::{self, Canvas},
};

/// Split a wire capture into (first header, decoded pixels) for each transmitted image.
fn images(wire: &str) -> Vec<(String, Vec<u8>)> {
    let mut result: Vec<(String, String)> = Vec::new();
    for chunk in wire.split("\x1b_G").skip(1) {
        let (header, payload) = chunk.split_once(';').unwrap();
        let payload = payload.strip_suffix("\x1b\\").unwrap();
        assert!(payload.len() <= 4096);
        if header.starts_with("a=T") {
            result.push((header.to_owned(), payload.to_owned()));
        } else if header.starts_with("q=2,m=") {
            result.last_mut().unwrap().1.push_str(payload);
        }
    }
    result
        .into_iter()
        .map(|(header, encoded)| {
            let mut pixels = Vec::new();
            ZlibDecoder::new(&STANDARD.decode(encoded).unwrap()[..])
                .read_to_end(&mut pixels)
                .unwrap();
            (header, pixels)
        })
        .collect()
}

#[test]
fn graphics_sends_zlib_raw_rgb_with_dimensions_and_cell_box() {
    let frame = Frame {
        width: 2,
        height: 1,
        rgb: vec![255, 0, 0, 0, 0, 255],
    };
    let view = Viewport {
        x: 3,
        y: 1,
        cols: 12,
        rows: 6,
        width: 2,
        height: 1,
    };
    let mut output = Vec::new();
    Canvas::default()
        .draw(&mut output, &frame, view, (1000, 1000), true, false)
        .unwrap();
    let wire = String::from_utf8(output).unwrap();
    let images = images(&wire);
    assert_eq!(images.len(), 1);
    assert!(
        images[0]
            .0
            .starts_with("a=T,f=24,o=z,s=2,v=1,i=31,p=1,q=2,C=1,c=12,r=6,m=0")
    );
    assert_eq!(images[0].1, [255, 0, 0, 0, 0, 255]);
}

#[test]
fn large_images_use_complete_bounded_chunks() {
    let frame = Frame {
        width: 96,
        height: 96,
        rgb: {
            let mut state = 1234567_u32;
            (0..96 * 96 * 3)
                .map(|_| {
                    state ^= state << 13;
                    state ^= state >> 17;
                    state ^= state << 5;
                    state as u8
                })
                .collect()
        },
    };
    let view = Viewport::fit(80, 24, 96, 96, 0.5);
    let mut output = Vec::new();
    Canvas::default()
        .draw(&mut output, &frame, view, (1000, 1000), false, true)
        .unwrap();
    let wire = String::from_utf8(output).unwrap();
    let chunks: Vec<_> = wire.split("\x1b_G").skip(1).collect();
    assert!(chunks.len() > 1);
    for (index, chunk) in chunks.iter().enumerate() {
        let header = chunk.split_once(';').unwrap().0;
        assert!(header.ends_with(if index + 1 == chunks.len() {
            "m=0"
        } else {
            "m=1"
        }));
    }
    let pixels = &images(&wire)[0].1;
    assert_eq!(pixels.len(), 96 * 96 * 3);
    assert!(pixels.chunks_exact(3).all(|p| p[0] == p[1] && p[1] == p[2]));
}

#[test]
fn frames_are_scaled_down_to_the_displayed_pixels_and_never_up() {
    assert_eq!(graphics::fit_pixels((1080, 2048), (400, 1000)), (400, 758));
    assert_eq!(graphics::fit_pixels((1080, 2048), (1000, 500)), (263, 500));
    assert_eq!(graphics::fit_pixels((100, 200), (1000, 1000)), (100, 200));
    assert_eq!(graphics::fit_pixels((100, 200), (0, 0)), (100, 200));

    // Each 2×2 block averages to one pixel.
    let frame = Frame {
        width: 4,
        height: 2,
        rgb: [[0, 0, 0], [100, 100, 100], [200, 0, 0], [200, 0, 0]]
            .iter()
            .chain(&[[100, 100, 100], [0, 0, 0], [0, 0, 200], [0, 0, 200]])
            .flatten()
            .copied()
            .collect(),
    };
    assert_eq!(
        graphics::scale(&frame, (2, 1), true, false),
        [50, 50, 50, 100, 0, 100]
    );
    let view = Viewport::fit(80, 24, 4, 2, 0.5);
    let mut output = Vec::new();
    Canvas::default()
        .draw(&mut output, &frame, view, (2, 1), true, false)
        .unwrap();
    let images = images(&String::from_utf8(output).unwrap());
    assert!(images[0].0.contains("s=2,v=1,"));
    assert_eq!(images[0].1, [50, 50, 50, 100, 0, 100]);
}

#[test]
fn each_frame_is_placed_before_the_previous_image_is_deleted() {
    let frame = Frame {
        width: 1,
        height: 1,
        rgb: vec![1, 2, 3],
    };
    let view = Viewport::fit(10, 10, 1, 1, 0.5);
    let mut canvas = Canvas::default();
    let mut first = Vec::new();
    canvas
        .draw(&mut first, &frame, view, (0, 0), true, false)
        .unwrap();
    assert!(!String::from_utf8(first).unwrap().contains("a=d"));
    for (new, old) in [(32, 31), (31, 32)] {
        let mut output = Vec::new();
        canvas
            .draw(&mut output, &frame, view, (0, 0), true, false)
            .unwrap();
        let wire = String::from_utf8(output).unwrap();
        let placed = wire.find(&format!("i={new},")).unwrap();
        let deleted = wire.find(&format!("a=d,d=I,i={old},")).unwrap();
        assert!(placed < deleted);
    }
    assert!(graphics::DELETE.contains("i=31,") && graphics::DELETE.contains("i=32,"));
}
