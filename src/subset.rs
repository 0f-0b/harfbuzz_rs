//! Font subsetting.
//!
//! Subsetting creates a new font that only contains the data needed to render a
//! chosen set of characters or glyphs. This is useful e.g. to embed fonts into
//! documents or to reduce the size of web fonts.
//!
//! To subset a font, describe what to keep using a [`SubsetInput`] and then
//! call [`Face::subset`]:
//!
//! ```
//! use harfbuzz_rs::subset::SubsetInput;
//! use harfbuzz_rs::Face;
//!
//! # fn try_main() -> Result<(), Box<dyn std::error::Error>> {
//! let face = Face::from_file("testfiles/SourceSansVariable-Roman.ttf", 0)?;
//!
//! let mut input = SubsetInput::new().expect("failed to allocate subset input");
//! input.add_text("Hello World!");
//!
//! let subset = face.subset(&input).expect("subsetting failed");
//! assert!(subset.glyph_count() < face.glyph_count());
//!
//! // The bytes of the new font file can be written to disk or embedded.
//! let font_file = subset.face_data();
//! assert!(font_file.len() < face.face_data().len());
//! # Ok(())
//! # }
//! # try_main().unwrap();
//! ```

use std::ptr::NonNull;

use crate::bindings::*;
use crate::common::{HarfbuzzObject, Owned, Tag};
use crate::face::Face;

bitflags! {
    /// Boolean options that change how a font is subsetted.
    ///
    /// Set them with [`SubsetInput::set_flags`].
    #[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct SubsetFlags: u32 {
        /// Drop hinting instructions from the subset.
        const NO_HINTING = HB_SUBSET_FLAGS_NO_HINTING;
        /// Keep glyph indices unchanged. Dropped glyphs are replaced by empty
        /// glyphs.
        const RETAIN_GIDS = HB_SUBSET_FLAGS_RETAIN_GIDS;
        /// Remove subroutines from the glyphs of a CFF font.
        const DESUBROUTINIZE = HB_SUBSET_FLAGS_DESUBROUTINIZE;
        /// Retain non-Unicode name records.
        const NAME_LEGACY = HB_SUBSET_FLAGS_NAME_LEGACY;
        /// Set the `OVERLAP_SIMPLE` flag on each simple glyph.
        const SET_OVERLAPS_FLAG = HB_SUBSET_FLAGS_SET_OVERLAPS_FLAG;
        /// Pass unrecognized tables through untouched instead of dropping them.
        const PASSTHROUGH_UNRECOGNIZED = HB_SUBSET_FLAGS_PASSTHROUGH_UNRECOGNIZED;
        /// Retain the outline of the `.notdef` glyph.
        const NOTDEF_OUTLINE = HB_SUBSET_FLAGS_NOTDEF_OUTLINE;
        /// Retain PostScript glyph names.
        const GLYPH_NAMES = HB_SUBSET_FLAGS_GLYPH_NAMES;
        /// Do not recalculate the Unicode ranges in the `OS/2` table.
        const NO_PRUNE_UNICODE_RANGES = HB_SUBSET_FLAGS_NO_PRUNE_UNICODE_RANGES;
        /// Do not perform glyph closure over layout (`GSUB`) substitution rules.
        const NO_LAYOUT_CLOSURE = HB_SUBSET_FLAGS_NO_LAYOUT_CLOSURE;
        /// Optimize the IUP deltas of the remaining `gvar` table.
        const OPTIMIZE_IUP_DELTAS = HB_SUBSET_FLAGS_OPTIMIZE_IUP_DELTAS;
        /// Do not pull mirrored versions of the input codepoints into the subset.
        const NO_BIDI_CLOSURE = HB_SUBSET_FLAGS_NO_BIDI_CLOSURE;
        /// Convert the output `CFF2` table to `CFF` when all axes of a variable
        /// font are pinned.
        const DOWNGRADE_CFF2 = HB_SUBSET_FLAGS_DOWNGRADE_CFF2;
        /// Renumber the charset of a CID-keyed CFF font sequentially.
        const CFF_IDENTITY_CHARSET = HB_SUBSET_FLAGS_CFF_IDENTITY_CHARSET;
    }
}

/// Describes what a subsetting operation should keep.
///
/// A new `SubsetInput` keeps nothing but the HarfBuzz defaults (e.g. the
/// `.notdef` glyph and a standard set of layout features). Add the characters
/// or glyphs you need with methods such as [`add_text`](Self::add_text) and
/// then apply it to a face using [`Face::subset`].
///
/// See the [module-level documentation](self) for an example.
#[derive(Debug)]
pub struct SubsetInput {
    raw: NonNull<hb_subset_input_t>,
}

impl SubsetInput {
    /// Creates a new subset input with default settings.
    ///
    /// Returns `None` if HarfBuzz fails to allocate the object.
    pub fn new() -> Option<Owned<SubsetInput>> {
        let raw = unsafe { hb_subset_input_create_or_fail() };
        if raw.is_null() {
            None
        } else {
            Some(unsafe { Owned::from_raw(raw) })
        }
    }

    /// Configures the input to keep all data of the font.
    ///
    /// This retains all glyphs, Unicode codepoints, layout features, name
    /// records and tables and sets the flags needed to leave the font intact.
    /// Useful as a starting point when you only want to e.g. pin variation
    /// axes.
    pub fn keep_everything(&mut self) {
        unsafe { hb_subset_input_keep_everything(self.as_raw_mut()) }
    }

    /// Returns the currently configured flags.
    pub fn flags(&self) -> SubsetFlags {
        SubsetFlags::from_bits_retain(unsafe { hb_subset_input_get_flags(self.as_raw()) })
    }

    /// Replaces the configured flags.
    pub fn set_flags(&mut self, flags: SubsetFlags) {
        unsafe { hb_subset_input_set_flags(self.as_raw_mut(), flags.bits()) }
    }

    /// Adds the given characters to the set of characters to keep.
    pub fn add_unicodes(&mut self, chars: impl IntoIterator<Item = char>) {
        self.extend_set(HB_SUBSET_SETS_UNICODE, chars.into_iter().map(u32::from));
    }

    /// Adds all characters of `text` to the set of characters to keep.
    pub fn add_text(&mut self, text: &str) {
        self.add_unicodes(text.chars());
    }

    /// Adds all codepoints from `first` to `last` (both inclusive) to the set
    /// of characters to keep.
    pub fn add_unicode_range(&mut self, first: char, last: char) {
        unsafe {
            let set = hb_subset_input_unicode_set(self.as_raw_mut());
            hb_set_add_range(set, first as u32, last as u32);
        }
    }

    /// Adds the given glyph ids to the set of glyphs to keep.
    pub fn add_glyphs(&mut self, glyphs: impl IntoIterator<Item = u32>) {
        self.extend_set(HB_SUBSET_SETS_GLYPH_INDEX, glyphs);
    }

    /// Adds tables to the set of tables that are dropped from the subset.
    pub fn drop_tables(&mut self, tables: impl IntoIterator<Item = Tag>) {
        self.extend_set(
            HB_SUBSET_SETS_DROP_TABLE_TAG,
            tables.into_iter().map(|t| t.0),
        );
    }

    /// Adds tables that are copied to the subset as-is instead of being
    /// subsetted.
    pub fn keep_tables_unsubsetted(&mut self, tables: impl IntoIterator<Item = Tag>) {
        self.extend_set(
            HB_SUBSET_SETS_NO_SUBSET_TABLE_TAG,
            tables.into_iter().map(|t| t.0),
        );
    }

    /// Adds OpenType layout features that are retained in the subset.
    pub fn add_layout_features(&mut self, features: impl IntoIterator<Item = Tag>) {
        self.extend_set(
            HB_SUBSET_SETS_LAYOUT_FEATURE_TAG,
            features.into_iter().map(|t| t.0),
        );
    }

    /// Adds OpenType layout scripts that are retained in the subset.
    ///
    /// By default all scripts are retained.
    pub fn add_layout_scripts(&mut self, scripts: impl IntoIterator<Item = Tag>) {
        self.extend_set(
            HB_SUBSET_SETS_LAYOUT_SCRIPT_TAG,
            scripts.into_iter().map(|t| t.0),
        );
    }

    /// Adds `name` table name ids that are retained in the subset.
    pub fn add_name_ids(&mut self, name_ids: impl IntoIterator<Item = u32>) {
        self.extend_set(HB_SUBSET_SETS_NAME_ID, name_ids);
    }

    /// Adds `name` table language ids that are retained in the subset.
    pub fn add_name_languages(&mut self, language_ids: impl IntoIterator<Item = u32>) {
        self.extend_set(HB_SUBSET_SETS_NAME_LANG_ID, language_ids);
    }

    /// Pins all variation axes of `face` to their default location, turning a
    /// variable font into a static instance.
    ///
    /// Returns `false` if the operation failed.
    pub fn pin_all_axes_to_default(&mut self, face: &Face<'_>) -> bool {
        unsafe { hb_subset_input_pin_all_axes_to_default(self.as_raw_mut(), face.as_raw()) != 0 }
    }

    /// Pins the variation axis `axis_tag` of `face` to its default location.
    ///
    /// Returns `false` if the axis does not exist or the operation failed.
    pub fn pin_axis_to_default(&mut self, face: &Face<'_>, axis_tag: impl Into<Tag>) -> bool {
        unsafe {
            hb_subset_input_pin_axis_to_default(self.as_raw_mut(), face.as_raw(), axis_tag.into().0)
                != 0
        }
    }

    /// Pins the variation axis `axis_tag` of `face` to `value`.
    ///
    /// Returns `false` if the axis does not exist or the operation failed.
    pub fn pin_axis_location(
        &mut self,
        face: &Face<'_>,
        axis_tag: impl Into<Tag>,
        value: f32,
    ) -> bool {
        unsafe {
            hb_subset_input_pin_axis_location(
                self.as_raw_mut(),
                face.as_raw(),
                axis_tag.into().0,
                value,
            ) != 0
        }
    }

    /// Restricts the variation axis `axis_tag` of `face` to the range
    /// `min..=max` with the new default location `default`.
    ///
    /// Returns `false` if the axis does not exist or the range is invalid.
    pub fn set_axis_range(
        &mut self,
        face: &Face<'_>,
        axis_tag: impl Into<Tag>,
        min: f32,
        max: f32,
        default: f32,
    ) -> bool {
        unsafe {
            hb_subset_input_set_axis_range(
                self.as_raw_mut(),
                face.as_raw(),
                axis_tag.into().0,
                min,
                max,
                default,
            ) != 0
        }
    }

    /// Returns the `(min, max, default)` range configured for the axis
    /// `axis_tag`, or `None` if the axis has not been restricted.
    pub fn axis_range(&self, axis_tag: impl Into<Tag>) -> Option<(f32, f32, f32)> {
        let (mut min, mut max, mut default) = (0.0, 0.0, 0.0);
        let found = unsafe {
            hb_subset_input_get_axis_range(
                self.as_raw(),
                axis_tag.into().0,
                &mut min,
                &mut max,
                &mut default,
            )
        };
        (found != 0).then_some((min, max, default))
    }

    fn extend_set(&mut self, set_type: hb_subset_sets_t, values: impl IntoIterator<Item = u32>) {
        let set = unsafe { hb_subset_input_set(self.as_raw_mut(), set_type) };
        for value in values {
            unsafe { hb_set_add(set, value) };
        }
    }
}

impl<'a> Face<'a> {
    /// Creates a new face containing only the data selected by `input`.
    ///
    /// Returns `None` if subsetting fails, e.g. because the face is invalid or
    /// is in a format that cannot be subsetted.
    ///
    /// The data of the new font file is available via
    /// [`face_data`](Face::face_data).
    ///
    /// # Examples
    ///
    /// ```
    /// use harfbuzz_rs::subset::{SubsetFlags, SubsetInput};
    /// use harfbuzz_rs::{Face, Tag};
    ///
    /// let face = Face::from_bytes(include_bytes!("../testfiles/SourceSansVariable-Roman.ttf"), 0);
    ///
    /// let mut input = SubsetInput::new().unwrap();
    /// input.add_text("abc");
    /// input.set_flags(SubsetFlags::NO_HINTING | SubsetFlags::RETAIN_GIDS);
    /// input.pin_all_axes_to_default(&face);
    ///
    /// let subset = face.subset(&input).unwrap();
    /// // The variable font was turned into a static instance.
    /// assert!(subset.table_with_tag(Tag::new('f', 'v', 'a', 'r')).is_none());
    /// ```
    pub fn subset(&self, input: &SubsetInput) -> Option<Owned<Face<'static>>> {
        let raw = unsafe { hb_subset_or_fail(self.as_raw(), input.as_raw()) };
        if raw.is_null() {
            None
        } else {
            Some(unsafe { Owned::from_raw(raw) })
        }
    }
}

unsafe impl HarfbuzzObject for SubsetInput {
    type Raw = hb_subset_input_t;

    unsafe fn from_raw(raw: *const hb_subset_input_t) -> Self {
        SubsetInput {
            raw: NonNull::new(raw as *mut _).unwrap(),
        }
    }

    fn as_raw(&self) -> *mut Self::Raw {
        self.raw.as_ptr()
    }

    unsafe fn reference(&self) {
        hb_subset_input_reference(self.as_raw());
    }

    unsafe fn dereference(&self) {
        hb_subset_input_destroy(self.as_raw());
    }
}

unsafe impl Send for SubsetInput {}
unsafe impl Sync for SubsetInput {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{shape, Font, UnicodeBuffer};

    const FONT: &[u8] = include_bytes!("../testfiles/SourceSansVariable-Roman.ttf");

    fn subset_with(configure: impl FnOnce(&Face<'_>, &mut SubsetInput)) -> Owned<Face<'static>> {
        let face = Face::from_bytes(FONT, 0);
        let mut input = SubsetInput::new().unwrap();
        configure(&face, &mut input);
        face.subset(&input).expect("subsetting failed")
    }

    fn advances(face: Owned<Face<'_>>, text: &str) -> Vec<i32> {
        let font = Font::new(face);
        let output = shape(&font, UnicodeBuffer::new().add_str(text), &[]);
        output
            .get_glyph_positions()
            .iter()
            .map(|p| p.x_advance)
            .collect()
    }

    #[test]
    fn subset_reduces_size() {
        let original = Face::from_bytes(FONT, 0);
        let subset = subset_with(|_, input| input.add_text("Hello"));

        assert!(subset.glyph_count() < original.glyph_count());
        assert!(subset.face_data().len() < original.face_data().len() / 2);
    }

    #[test]
    fn subset_preserves_shaping() {
        let subset = subset_with(|_, input| input.add_text("Hello"));
        let original = Face::from_bytes(FONT, 0);
        assert_eq!(advances(subset, "Hello"), advances(original, "Hello"));
    }

    #[test]
    fn subset_output_is_a_valid_font() {
        let subset = subset_with(|_, input| input.add_text("Hello"));
        let data: Vec<u8> = subset.face_data().as_ref().to_vec();
        let reloaded = Face::from_bytes(&data, 0);
        assert_eq!(reloaded.glyph_count(), subset.glyph_count());
        assert!(reloaded.table_with_tag(b"glyf").is_some());
    }

    #[test]
    fn unicode_range_and_glyphs() {
        let by_range = subset_with(|_, input| input.add_unicode_range('a', 'z'));
        let by_chars = subset_with(|_, input| input.add_unicodes('a'..='z'));
        assert_eq!(by_range.glyph_count(), by_chars.glyph_count());

        // `.notdef` plus the glyphs we asked for.
        let by_glyphs = subset_with(|_, input| input.add_glyphs([5, 6, 7]));
        assert_eq!(by_glyphs.glyph_count(), 4);
    }

    #[test]
    fn retain_gids_keeps_glyph_ids() {
        let subset = subset_with(|_, input| {
            input.add_text("A");
            input.set_flags(SubsetFlags::RETAIN_GIDS);
        });
        let original = Face::from_bytes(FONT, 0);
        let gid = |face: Owned<Face<'_>>| {
            let font = Font::new(face);
            font.get_nominal_glyph('A')
        };
        assert_eq!(gid(subset), gid(original));
    }

    #[test]
    fn flags_roundtrip() {
        let mut input = SubsetInput::new().unwrap();
        assert_eq!(input.flags(), SubsetFlags::empty());
        let flags = SubsetFlags::NO_HINTING | SubsetFlags::GLYPH_NAMES;
        input.set_flags(flags);
        assert_eq!(input.flags(), flags);
    }

    #[test]
    fn drop_tables_removes_them() {
        let gpos = Tag::new('G', 'P', 'O', 'S');
        let with = subset_with(|_, input| input.add_text("Hello"));
        let without = subset_with(|_, input| {
            input.add_text("Hello");
            input.drop_tables([gpos]);
        });
        assert!(with.table_with_tag(gpos).is_some());
        assert!(without.table_with_tag(gpos).is_none());
    }

    #[test]
    fn pinning_axes_creates_static_font() {
        let fvar = Tag::new('f', 'v', 'a', 'r');
        let variable = subset_with(|_, input| input.add_text("Hello"));
        assert!(variable.table_with_tag(fvar).is_some());

        let static_font = subset_with(|face, input| {
            input.add_text("Hello");
            assert!(input.pin_all_axes_to_default(face));
        });
        assert!(static_font.table_with_tag(fvar).is_none());
    }

    #[test]
    fn axis_range() {
        let wght = Tag::new('w', 'g', 'h', 't');
        let face = Face::from_bytes(FONT, 0);
        let mut input = SubsetInput::new().unwrap();
        assert_eq!(input.axis_range(wght), None);

        assert!(input.set_axis_range(&face, wght, 300.0, 600.0, 400.0));
        assert_eq!(input.axis_range(wght), Some((300.0, 600.0, 400.0)));

        assert!(input.pin_axis_location(&face, wght, 500.0));
        assert_eq!(input.axis_range(wght), Some((500.0, 500.0, 500.0)));

        // The font has no axis named `abcd`.
        assert!(!input.pin_axis_to_default(&face, b"abcd"));
    }

    #[test]
    fn subsetting_an_invalid_face_fails() {
        let mut input = SubsetInput::new().unwrap();
        input.add_text("abc");
        let face = Face::from_bytes(b"not a font", 0);
        assert!(face.subset(&input).is_none());
    }
}
