use harfbuzz_rs::{shape, Face, Font, UnicodeBuffer};

// Execute this file from the root directory of this repository.
//
// The output should look like the following:
// gid09=00@ 634,0+0
// gid32=01@ 478,0+0
// gid39=02@ 230,0+0
// gid39=03@ 230,0+0
// gid42=04@ 520,0+0
// gid01=05@ 200,0+0
// gid24=06@ 764,0+0
// gid42=07@ 532,0+0
// gid45=08@ 306,0+0
// gid39=09@ 230,0+0
// gid31=10@ 540,0+0
// gid1146=11@ 248,0+0
fn main() {
    let index = 0;
    let path = "testfiles/SourceSansVariable-Roman.ttf";
    let face = Face::from_file(path, index).expect("Error reading font file.");
    let font = Font::new(face);

    // Create a buffer with some text, shape it...
    let buffer = UnicodeBuffer::new().add_str("Hello World!");

    let result = shape(&font, buffer, &[]);

    // ... and get the results.
    let positions = result.get_glyph_positions();
    let infos = result.get_glyph_infos();

    // iterate over the shaped glyphs
    for (position, info) in positions.iter().zip(infos) {
        let gid = info.codepoint;
        let cluster = info.cluster;
        let x_advance = position.x_advance;
        let x_offset = position.x_offset;
        let y_offset = position.y_offset;

        println!(
            "gid{gid:0>2?}={cluster:0>2?}@{x_advance:>4?},{x_offset:?}+{y_offset:?}"
        );
    }
}
