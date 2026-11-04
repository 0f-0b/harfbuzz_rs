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
//! use harfbuzz_rs::subset::{SubsetInput};
//! use harfbuzz_rs::Face;
//!
//! # fn try_main() -> Result<(), Box<dyn std::error::Error>> {
//! let face = Face::from_file("testfiles/SourceSansVariable-Roman.ttf", 0)?;
//!
//! let mut input = SubsetInput::new()?;
//! input.unicode_set_mut().insert_str("Hello World!");
//!
//! let subset = face.subset(&input)?;
//! assert!(subset.glyph_count() < face.glyph_count());
//!
//! // The bytes of the new font file can be written to disk or embedded.
//! let font_file = subset.face_data();
//! assert!(font_file.len() < face.face_data().len());
//! # Ok(())
//! # }
//! # try_main().unwrap();
//! ```

use std::fmt;
use std::ptr::NonNull;

use crate::bindings::*;
use crate::common::{Borrowed, BorrowedMut, HarfbuzzObject, Owned, Tag};
use crate::face::Face;
use crate::set::{CharSet, Set, TagSet};

/// The error returned when a subsetting operation fails.
///
/// HarfBuzz does not report the reason of a failure. Typical causes are an
/// invalid or unsupported font, a variation axis that does not exist or an
/// allocation failure.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct SubsetError;

impl fmt::Display for SubsetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "font subsetting failed")
    }
}

impl std::error::Error for SubsetError {}

/// Converts a `hb_bool_t` returned by HarfBuzz into a `Result`.
fn check(success: hb_bool_t) -> Result<(), SubsetError> {
    if success != 0 {
        Ok(())
    } else {
        Err(SubsetError)
    }
}

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
/// `.notdef` glyph and a standard set of layout features). Describe what else
/// to keep by modifying its sets and then apply the input to a face using
/// [`Face::subset`].
///
/// There is one accessor pair for each set, e.g.
/// [`unicode_set`](Self::unicode_set) and
/// [`unicode_set_mut`](Self::unicode_set_mut). The sets come in three kinds:
///
/// - [`CharSet`]: Unicode codepoints ([`unicode_set`](Self::unicode_set)),
/// - [`TagSet`]: OpenType tags ([`drop_table_set`](Self::drop_table_set),
///   [`no_subset_table_set`](Self::no_subset_table_set),
///   [`layout_feature_set`](Self::layout_feature_set) and
///   [`layout_script_set`](Self::layout_script_set)),
/// - [`Set`] of `u32`: glyph ids and name ids
///   ([`glyph_set`](Self::glyph_set), [`name_id_set`](Self::name_id_set) and
///   [`name_lang_id_set`](Self::name_lang_id_set)).
///
/// # Examples
///
/// ```
/// use harfbuzz_rs::subset::SubsetInput;
/// use harfbuzz_rs::Tag;
///
/// let mut input = SubsetInput::new().unwrap();
///
/// // Keep a range of characters, but not the letter `X`.
/// input.unicode_set_mut().insert_range('A'..='Z');
/// input.unicode_set_mut().remove('X');
/// assert_eq!(input.unicode_set().len(), 25);
///
/// // Always keep glyph 7.
/// input.glyph_set_mut().insert(7);
///
/// // Drop the `GPOS` table.
/// input.drop_table_set_mut().insert(Tag::new('G', 'P', 'O', 'S'));
/// ```
///
/// See the [module-level documentation](self) for an example.
#[derive(Debug)]
pub struct SubsetInput {
    raw: NonNull<hb_subset_input_t>,
}

impl SubsetInput {
    /// Creates a new subset input with default settings.
    ///
    /// Returns an error if HarfBuzz fails to allocate the object.
    pub fn new() -> Result<Owned<SubsetInput>, SubsetError> {
        let raw = unsafe { hb_subset_input_create_or_fail() };
        if raw.is_null() {
            Err(SubsetError)
        } else {
            Ok(unsafe { Owned::from_raw(raw) })
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

    /// Pins all variation axes of `face` to their default location, turning a
    /// variable font into a static instance.
    ///
    /// Returns an error if the operation failed.
    pub fn pin_all_axes_to_default(&mut self, face: &Face<'_>) -> Result<(), SubsetError> {
        check(unsafe { hb_subset_input_pin_all_axes_to_default(self.as_raw_mut(), face.as_raw()) })
    }

    /// Pins the variation axis `axis_tag` of `face` to its default location.
    ///
    /// Returns an error if the axis does not exist or the operation failed.
    pub fn pin_axis_to_default(
        &mut self,
        face: &Face<'_>,
        axis_tag: impl Into<Tag>,
    ) -> Result<(), SubsetError> {
        check(unsafe {
            hb_subset_input_pin_axis_to_default(self.as_raw_mut(), face.as_raw(), axis_tag.into().0)
        })
    }

    /// Pins the variation axis `axis_tag` of `face` to `value`.
    ///
    /// Returns an error if the axis does not exist or the operation failed.
    pub fn pin_axis_location(
        &mut self,
        face: &Face<'_>,
        axis_tag: impl Into<Tag>,
        value: f32,
    ) -> Result<(), SubsetError> {
        check(unsafe {
            hb_subset_input_pin_axis_location(
                self.as_raw_mut(),
                face.as_raw(),
                axis_tag.into().0,
                value,
            )
        })
    }

    /// Restricts the variation axis `axis_tag` of `face` to the range
    /// `min..=max` with the new default location `default`.
    ///
    /// Returns an error if the axis does not exist or the range is invalid.
    pub fn set_axis_range(
        &mut self,
        face: &Face<'_>,
        axis_tag: impl Into<Tag>,
        min: f32,
        max: f32,
        default: f32,
    ) -> Result<(), SubsetError> {
        check(unsafe {
            hb_subset_input_set_axis_range(
                self.as_raw_mut(),
                face.as_raw(),
                axis_tag.into().0,
                min,
                max,
                default,
            )
        })
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

    /// Returns a read-only view of one of the sets of this input.
    fn borrow_set<T: HarfbuzzObject<Raw = hb_set_t>>(
        &self,
        kind: hb_subset_sets_t,
    ) -> Borrowed<'_, T> {
        // The borrow of `self` guarantees that the set is not mutated while
        // the returned `Borrowed` is alive.
        unsafe {
            let raw = hb_subset_input_set(self.as_raw(), kind);
            Borrowed::from_raw_borrowed(raw)
        }
    }

    /// Returns a mutable view of one of the sets of this input.
    fn borrow_set_mut<T: HarfbuzzObject<Raw = hb_set_t>>(
        &mut self,
        kind: hb_subset_sets_t,
    ) -> BorrowedMut<'_, T> {
        // The exclusive borrow of `self` guarantees that nothing else accesses
        // the set while the returned `BorrowedMut` is alive.
        unsafe {
            let raw = hb_subset_input_set(self.as_raw_mut(), kind);
            BorrowedMut::from_raw_borrowed(raw)
        }
    }
}

macro_rules! set_accessors {
    ($($doc:literal $get:ident, $get_mut:ident: $set:ty = $kind:ident;)*) => {
        impl SubsetInput {$(
            #[doc = concat!("Returns the set of ", $doc, ".")]
            pub fn $get(&self) -> Borrowed<'_, $set> {
                self.borrow_set($kind)
            }

            #[doc = concat!("Returns the set of ", $doc, " for modification.")]
            pub fn $get_mut(&mut self) -> BorrowedMut<'_, $set> {
                self.borrow_set_mut($kind)
            }
        )*}
    };
}

set_accessors! {
    "glyph ids to retain"
        glyph_set, glyph_set_mut: Set = HB_SUBSET_SETS_GLYPH_INDEX;
    "Unicode codepoints to retain"
        unicode_set, unicode_set_mut: CharSet = HB_SUBSET_SETS_UNICODE;
    "tags of tables that are copied without being subsetted"
        no_subset_table_set, no_subset_table_set_mut: TagSet = HB_SUBSET_SETS_NO_SUBSET_TABLE_TAG;
    "tags of tables that are dropped"
        drop_table_set, drop_table_set_mut: TagSet = HB_SUBSET_SETS_DROP_TABLE_TAG;
    "`name` table name ids to retain"
        name_id_set, name_id_set_mut: Set = HB_SUBSET_SETS_NAME_ID;
    "`name` table language ids to retain"
        name_lang_id_set, name_lang_id_set_mut: Set = HB_SUBSET_SETS_NAME_LANG_ID;
    "tags of layout features to retain"
        layout_feature_set, layout_feature_set_mut: TagSet = HB_SUBSET_SETS_LAYOUT_FEATURE_TAG;
    "tags of layout scripts to retain"
        layout_script_set, layout_script_set_mut: TagSet = HB_SUBSET_SETS_LAYOUT_SCRIPT_TAG;
}

impl<'a> Face<'a> {
    /// Creates a new face containing only the data selected by `input`.
    ///
    /// Returns an error if subsetting fails, e.g. because the face is invalid or
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
    /// input.unicode_set_mut().insert_str("abc");
    /// input.set_flags(SubsetFlags::NO_HINTING | SubsetFlags::RETAIN_GIDS);
    /// input.pin_all_axes_to_default(&face).unwrap();
    ///
    /// let subset = face.subset(&input).unwrap();
    /// // The variable font was turned into a static instance.
    /// assert!(subset.table_with_tag(Tag::new('f', 'v', 'a', 'r')).is_none());
    /// ```
    pub fn subset(&self, input: &SubsetInput) -> Result<Owned<Face<'static>>, SubsetError> {
        let raw = unsafe { hb_subset_or_fail(self.as_raw(), input.as_raw()) };
        if raw.is_null() {
            Err(SubsetError)
        } else {
            Ok(unsafe { Owned::from_raw(raw) })
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
        let subset = subset_with(|_, input| input.unicode_set_mut().insert_str("Hello"));

        assert!(subset.glyph_count() < original.glyph_count());
        assert!(subset.face_data().len() < original.face_data().len() / 2);
    }

    #[test]
    fn subset_preserves_shaping() {
        let subset = subset_with(|_, input| input.unicode_set_mut().insert_str("Hello"));
        let original = Face::from_bytes(FONT, 0);
        assert_eq!(advances(subset, "Hello"), advances(original, "Hello"));
    }

    #[test]
    fn subset_output_is_a_valid_font() {
        let subset = subset_with(|_, input| input.unicode_set_mut().insert_str("Hello"));
        let data: Vec<u8> = subset.face_data().as_ref().to_vec();
        let reloaded = Face::from_bytes(&data, 0);
        assert_eq!(reloaded.glyph_count(), subset.glyph_count());
        assert!(reloaded.table_with_tag(b"glyf").is_some());
    }

    #[test]
    fn unicode_range_and_glyphs() {
        let by_range = subset_with(|_, input| input.unicode_set_mut().insert_range('a'..='z'));
        let by_chars = subset_with(|_, input| {
            input
                .unicode_set_mut()
                .insert_str("abcdefghijklmnopqrstuvwxyz")
        });
        assert_eq!(by_range.glyph_count(), by_chars.glyph_count());

        // `.notdef` plus the glyphs we asked for.
        let by_glyphs =
            subset_with(|_, input| input.glyph_set_mut().insert_sorted_slice(&[5, 6, 7]));
        assert_eq!(by_glyphs.glyph_count(), 4);
    }

    #[test]
    fn retain_gids_keeps_glyph_ids() {
        let subset = subset_with(|_, input| {
            input.unicode_set_mut().insert_str("A");
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
        let with = subset_with(|_, input| input.unicode_set_mut().insert_str("Hello"));
        let without = subset_with(|_, input| {
            input.unicode_set_mut().insert_str("Hello");
            input.drop_table_set_mut().insert(gpos);
        });
        assert!(with.table_with_tag(gpos).is_some());
        assert!(without.table_with_tag(gpos).is_none());
    }

    #[test]
    fn pinning_axes_creates_static_font() {
        let fvar = Tag::new('f', 'v', 'a', 'r');
        let variable = subset_with(|_, input| input.unicode_set_mut().insert_str("Hello"));
        assert!(variable.table_with_tag(fvar).is_some());

        let static_font = subset_with(|face, input| {
            input.unicode_set_mut().insert_str("Hello");
            input.pin_all_axes_to_default(face).unwrap();
        });
        assert!(static_font.table_with_tag(fvar).is_none());
    }

    #[test]
    fn axis_range() {
        let wght = Tag::new('w', 'g', 'h', 't');
        let face = Face::from_bytes(FONT, 0);
        let mut input = SubsetInput::new().unwrap();
        assert_eq!(input.axis_range(wght), None);

        input
            .set_axis_range(&face, wght, 300.0, 600.0, 400.0)
            .unwrap();
        assert_eq!(input.axis_range(wght), Some((300.0, 600.0, 400.0)));

        input.pin_axis_location(&face, wght, 500.0).unwrap();
        assert_eq!(input.axis_range(wght), Some((500.0, 500.0, 500.0)));

        // The font has no axis named `abcd`.
        assert_eq!(input.pin_axis_to_default(&face, b"abcd"), Err(SubsetError));
    }

    #[test]
    fn set_and_set_mut_access_the_same_sets() {
        let gpos = Tag::new('G', 'P', 'O', 'S');
        let mut input = SubsetInput::new().unwrap();
        input.unicode_set_mut().insert_str("ab");
        input.glyph_set_mut().insert(7);
        input.drop_table_set_mut().insert(gpos);

        let unicode = input.unicode_set();
        assert_eq!(unicode.iter().collect::<Vec<_>>(), ['a', 'b']);
        drop(unicode);
        assert!(input.glyph_set().contains(7));
        assert!(input.drop_table_set().contains(gpos));
    }

    #[test]
    fn set_mut_changes_the_subset() {
        let face = Face::from_bytes(FONT, 0);
        let mut input = SubsetInput::new().unwrap();
        input.unicode_set_mut().insert_range('a'..='z');
        let full = face.subset(&input).unwrap();

        input.unicode_set_mut().remove_range('b'..='z');
        let reduced = face.subset(&input).unwrap();
        assert!(reduced.glyph_count() < full.glyph_count());
    }

    #[test]
    fn swapping_borrowed_sets_is_memory_safe() {
        let mut first = SubsetInput::new().unwrap();
        let mut second = SubsetInput::new().unwrap();
        first.unicode_set_mut().insert_str("a");
        second.unicode_set_mut().insert_str("bc");

        std::mem::swap(
            &mut *first.unicode_set_mut(),
            &mut *second.unicode_set_mut(),
        );
        drop(second);
        // `first` still owns a valid set.
        assert!(first.unicode_set().contains('a'));
    }

    #[test]
    fn typed_sets_use_their_element_type() {
        let mut input = SubsetInput::new().unwrap();
        let gpos = Tag::new('G', 'P', 'O', 'S');
        let kern = Tag::new('k', 'e', 'r', 'n');
        input.layout_feature_set_mut().clear();
        input.layout_feature_set_mut().insert_slice(&[kern, gpos]);
        // HarfBuzz pre-fills this set with default name ids.
        input.name_id_set_mut().clear();
        input.name_id_set_mut().insert_sorted_slice(&[1, 2, 3]);

        let features = input.layout_feature_set();
        assert_eq!(features.iter().count(), 2);
        assert!(features.contains(kern));
        assert_eq!(input.name_id_set().len(), 3);
    }

    #[test]
    fn char_and_tag_sets_accept_sorted_slices() {
        let mut input = SubsetInput::new().unwrap();

        let mut chars = input.unicode_set_mut();
        chars.clear();
        chars.insert_sorted_slice(&['a', 'b', 'z', '\u{1F600}']);
        assert_eq!(
            chars.iter().collect::<Vec<_>>(),
            ['a', 'b', 'z', '\u{1F600}']
        );
        assert_eq!(chars.iter().count(), chars.len());
        drop(chars);

        let gpos = Tag::new('G', 'P', 'O', 'S');
        let kern = Tag::new('k', 'e', 'r', 'n');
        let mut tags = input.drop_table_set_mut();
        tags.clear();
        tags.insert_sorted_slice(&[gpos, kern]);
        assert!(tags.contains(gpos) && tags.contains(kern));
        assert_eq!(tags.len(), 2);
    }

    #[test]
    fn error_display() {
        assert_eq!(SubsetError.to_string(), "font subsetting failed");
    }

    #[test]
    fn subsetting_an_invalid_face_fails() {
        let mut input = SubsetInput::new().unwrap();
        input.unicode_set_mut().insert_str("abc");
        let face = Face::from_bytes(b"not a font", 0);
        assert_eq!(face.subset(&input).err(), Some(SubsetError));
    }
}
