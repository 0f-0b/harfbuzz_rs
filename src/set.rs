//! Sets of integers.

use std::fmt;
use std::hash::{Hash, Hasher};
use std::iter::{FromIterator, FusedIterator};
use std::ops::RangeInclusive;
use std::ptr::NonNull;

use crate::bindings::*;
use crate::common::{HarfbuzzObject, Owned, Shared, Tag};

/// The value HarfBuzz uses to mark "no element" during iteration.
const VALUE_INVALID: u32 = u32::MAX;

/// A wrapper around `hb_set_t`.
///
/// A `Set` holds an ordered set of non-negative integers (`u32`). HarfBuzz uses
/// it to represent sets of Unicode codepoints, glyph ids, tags and similar
/// things. Internally it stores ranges of consecutive values compactly.
///
/// Sets that are known to contain only Unicode codepoints or only tags are
/// available as the more specific [`CharSet`] and [`TagSet`].
///
/// # Examples
///
/// ```
/// use harfbuzz_rs::Set;
///
/// let mut set = Set::new();
/// set.insert(3);
/// set.insert_range(10..=12);
///
/// assert!(set.contains(11));
/// assert_eq!(set.len(), 4);
/// assert_eq!(set.iter().collect::<Vec<_>>(), [3, 10, 11, 12]);
/// ```
///
/// Sets can also be collected from an iterator:
///
/// ```
/// use harfbuzz_rs::{Owned, Set};
///
/// let set: Owned<Set> = [1, 2, 3].iter().copied().collect();
/// assert_eq!(set.len(), 3);
/// ```
pub struct Set {
    raw: NonNull<hb_set_t>,
}

impl Set {
    /// Creates a new empty set.
    pub fn new() -> Owned<Set> {
        let raw = unsafe { hb_set_create() };
        unsafe { Owned::from_raw(raw) }
    }

    /// Returns the immutable empty set.
    pub fn empty() -> Shared<Set> {
        unsafe { Shared::from_raw_ref(hb_set_get_empty()) }
    }

    /// Returns a new set with the same elements as `self`.
    pub fn copy(&self) -> Owned<Set> {
        unsafe { Owned::from_raw(hb_set_copy(self.as_raw())) }
    }

    /// Returns `false` if an allocation failed during an earlier operation on
    /// this set, in which case its contents are unreliable.
    ///
    /// HarfBuzz's shared empty set ([`Set::empty`]) is an inert object for which
    /// this also returns `false`.
    pub fn allocation_successful(&self) -> bool {
        unsafe { hb_set_allocation_successful(self.as_raw()) != 0 }
    }

    /// Removes all elements from the set.
    pub fn clear(&mut self) {
        unsafe { hb_set_clear(self.as_raw_mut()) }
    }

    /// Returns `true` if the set contains no elements.
    pub fn is_empty(&self) -> bool {
        unsafe { hb_set_is_empty(self.as_raw()) != 0 }
    }

    /// Returns the number of elements in the set.
    pub fn len(&self) -> usize {
        unsafe { hb_set_get_population(self.as_raw()) as usize }
    }

    /// Returns `true` if `value` is an element of the set.
    pub fn contains(&self, value: u32) -> bool {
        unsafe { hb_set_has(self.as_raw(), value) != 0 }
    }

    /// Adds `value` to the set.
    ///
    /// # Examples
    ///
    /// ```
    /// use harfbuzz_rs::Set;
    ///
    /// let mut set = Set::new();
    /// set.insert(42);
    /// set.insert(42); // inserting a value twice has no further effect
    /// assert!(set.contains(42));
    /// assert_eq!(set.len(), 1);
    /// ```
    pub fn insert(&mut self, value: u32) {
        unsafe { hb_set_add(self.as_raw_mut(), value) }
    }

    /// Adds all values in `range` to the set.
    ///
    /// # Examples
    ///
    /// ```
    /// use harfbuzz_rs::Set;
    ///
    /// let mut set = Set::new();
    /// set.insert_range(10..=12);
    ///
    /// assert_eq!(set.len(), 3);
    /// assert!(set.contains(11));
    /// assert_eq!(set.ranges().collect::<Vec<_>>(), [10..=12]);
    /// ```
    pub fn insert_range(&mut self, range: RangeInclusive<u32>) {
        unsafe { hb_set_add_range(self.as_raw_mut(), *range.start(), *range.end()) }
    }

    /// Adds all values in `values` to the set, in any order.
    ///
    /// If `values` is sorted in ascending order,
    /// [`insert_sorted_slice`](Self::insert_sorted_slice) is faster.
    ///
    /// # Examples
    ///
    /// ```
    /// use harfbuzz_rs::Set;
    ///
    /// let mut set = Set::new();
    /// set.insert_slice(&[9, 1, 2, 1]);
    /// assert_eq!(set.iter().collect::<Vec<_>>(), [1, 2, 9]);
    /// ```
    pub fn insert_slice(&mut self, values: &[u32]) {
        for &value in values {
            self.insert(value);
        }
    }

    /// Adds all values in `values` to the set, which must be sorted.
    ///
    /// No copy of `values` is made. Use [`insert_slice`](Self::insert_slice) if
    /// the values might not be sorted.
    ///
    /// This is faster than inserting the values one by one, but `values` must
    /// be sorted in ascending order (repeated values are fine). If it is not,
    /// HarfBuzz stops at the first value that is smaller than its predecessor
    /// and ignores the rest. This can not cause memory unsafety, but the set
    /// will not contain all of `values`.
    ///
    /// # Examples
    ///
    /// ```
    /// use harfbuzz_rs::Set;
    ///
    /// let mut set = Set::new();
    /// set.insert_sorted_slice(&[1, 2, 2, 9]);
    /// assert_eq!(set.iter().collect::<Vec<_>>(), [1, 2, 9]);
    /// ```
    pub fn insert_sorted_slice(&mut self, values: &[u32]) {
        let len = values.len().min(std::os::raw::c_uint::MAX as usize);
        unsafe { hb_set_add_sorted_array(self.as_raw_mut(), values.as_ptr(), len as _) }
    }

    /// Removes `value` from the set.
    pub fn remove(&mut self, value: u32) {
        unsafe { hb_set_del(self.as_raw_mut(), value) }
    }

    /// Removes all values in `range` from the set.
    pub fn remove_range(&mut self, range: RangeInclusive<u32>) {
        unsafe { hb_set_del_range(self.as_raw_mut(), *range.start(), *range.end()) }
    }

    /// Returns the smallest element of the set or `None` if it is empty.
    pub fn min(&self) -> Option<u32> {
        Some(unsafe { hb_set_get_min(self.as_raw()) }).filter(|&v| v != VALUE_INVALID)
    }

    /// Returns the largest element of the set or `None` if it is empty.
    pub fn max(&self) -> Option<u32> {
        Some(unsafe { hb_set_get_max(self.as_raw()) }).filter(|&v| v != VALUE_INVALID)
    }

    /// Returns `true` if `self` and `other` have at least one element in
    /// common.
    pub fn intersects(&self, other: &Set) -> bool {
        unsafe { hb_set_intersects(self.as_raw(), other.as_raw()) != 0 }
    }

    /// Returns `true` if every element of `self` is also in `other`.
    pub fn is_subset(&self, other: &Set) -> bool {
        unsafe { hb_set_is_subset(self.as_raw(), other.as_raw()) != 0 }
    }

    /// Replaces the contents of `self` with those of `other`.
    pub fn assign(&mut self, other: &Set) {
        unsafe { hb_set_set(self.as_raw_mut(), other.as_raw()) }
    }

    /// Adds all elements of `other` to `self`.
    pub fn union_with(&mut self, other: &Set) {
        unsafe { hb_set_union(self.as_raw_mut(), other.as_raw()) }
    }

    /// Removes all elements from `self` that are not in `other`.
    pub fn intersect_with(&mut self, other: &Set) {
        unsafe { hb_set_intersect(self.as_raw_mut(), other.as_raw()) }
    }

    /// Removes all elements of `other` from `self`.
    pub fn subtract(&mut self, other: &Set) {
        unsafe { hb_set_subtract(self.as_raw_mut(), other.as_raw()) }
    }

    /// Keeps the elements that are in exactly one of `self` and `other`.
    pub fn symmetric_difference_with(&mut self, other: &Set) {
        unsafe { hb_set_symmetric_difference(self.as_raw_mut(), other.as_raw()) }
    }

    /// Inverts the set: afterwards it contains exactly the values it did not
    /// contain before.
    pub fn invert(&mut self) {
        unsafe { hb_set_invert(self.as_raw_mut()) }
    }

    /// Returns `true` if the set is inverted, i.e. it is stored as the
    /// complement of its elements.
    pub fn is_inverted(&self) -> bool {
        unsafe { hb_set_is_inverted(self.as_raw()) != 0 }
    }

    /// Returns an iterator over the elements of the set in ascending order.
    pub fn iter(&self) -> Iter<'_> {
        Iter {
            set: self,
            last: VALUE_INVALID,
        }
    }

    /// Returns an iterator over the maximal ranges of consecutive elements of
    /// the set in ascending order.
    ///
    /// # Examples
    ///
    /// ```
    /// use harfbuzz_rs::Set;
    ///
    /// let mut set = Set::new();
    /// set.insert_range(1..=3);
    /// set.insert(7);
    /// assert_eq!(set.ranges().collect::<Vec<_>>(), [1..=3, 7..=7]);
    /// ```
    pub fn ranges(&self) -> Ranges<'_> {
        Ranges {
            set: self,
            last: VALUE_INVALID,
        }
    }
}

/// An iterator over the elements of a [`Set`], created by [`Set::iter`].
#[derive(Debug, Clone)]
pub struct Iter<'a> {
    set: &'a Set,
    last: u32,
}

impl Iterator for Iter<'_> {
    type Item = u32;

    fn next(&mut self) -> Option<u32> {
        let found = unsafe { hb_set_next(self.set.as_raw(), &mut self.last) };
        (found != 0).then_some(self.last)
    }
}

impl FusedIterator for Iter<'_> {}

/// An iterator over the ranges of consecutive elements of a [`Set`], created
/// by [`Set::ranges`].
#[derive(Debug, Clone)]
pub struct Ranges<'a> {
    set: &'a Set,
    last: u32,
}

impl Iterator for Ranges<'_> {
    type Item = RangeInclusive<u32>;

    fn next(&mut self) -> Option<RangeInclusive<u32>> {
        let (mut first, mut last) = (self.last, self.last);
        let found = unsafe { hb_set_next_range(self.set.as_raw(), &mut first, &mut last) };
        self.last = last;
        (found != 0).then_some(first..=last)
    }
}

impl FusedIterator for Ranges<'_> {}

impl<'a> IntoIterator for &'a Set {
    type Item = u32;
    type IntoIter = Iter<'a>;

    fn into_iter(self) -> Iter<'a> {
        self.iter()
    }
}

impl Extend<u32> for Set {
    fn extend<I: IntoIterator<Item = u32>>(&mut self, iter: I) {
        for value in iter {
            self.insert(value);
        }
    }
}

impl FromIterator<u32> for Owned<Set> {
    fn from_iter<I: IntoIterator<Item = u32>>(iter: I) -> Self {
        let mut set = Set::new();
        set.extend(iter);
        set
    }
}

impl Default for Owned<Set> {
    fn default() -> Self {
        Set::new()
    }
}

impl PartialEq for Set {
    fn eq(&self, other: &Set) -> bool {
        unsafe { hb_set_is_equal(self.as_raw(), other.as_raw()) != 0 }
    }
}

impl Eq for Set {}

impl Hash for Set {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_u32(unsafe { hb_set_hash(self.as_raw()) });
    }
}

impl fmt::Debug for Set {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_set().entries(self.ranges()).finish()
    }
}

unsafe impl HarfbuzzObject for Set {
    type Raw = hb_set_t;

    unsafe fn from_raw(raw: *const hb_set_t) -> Self {
        Set {
            raw: NonNull::new(raw as *mut _).unwrap(),
        }
    }

    fn as_raw(&self) -> *mut Self::Raw {
        self.raw.as_ptr()
    }

    unsafe fn reference(&self) {
        hb_set_reference(self.as_raw());
    }

    unsafe fn dereference(&self) {
        hb_set_destroy(self.as_raw());
    }
}

unsafe impl Send for Set {}
unsafe impl Sync for Set {}

/// Implements the parts that `CharSet` and `TagSet` have in common.
///
/// The type must be a struct with a single `set: Set` field and provide the
/// functions `to_raw` and `from_raw_value` to convert between `$elem` and the
/// `u32` stored in the set. `$elem` must have the same size, alignment and
/// value representation as `u32`.
macro_rules! typed_set {
    ($name:ident, $elem:ty) => {
        const _: () = assert!(
            std::mem::size_of::<$elem>() == std::mem::size_of::<u32>()
                && std::mem::align_of::<$elem>() == std::mem::align_of::<u32>()
        );

        impl $name {
            /// Returns `true` if `value` is an element of the set.
            pub fn contains(&self, value: $elem) -> bool {
                self.set.contains(Self::to_raw(value))
            }

            /// Adds `value` to the set.
            pub fn insert(&mut self, value: $elem) {
                self.set.insert(Self::to_raw(value))
            }

            /// Adds all values in `range` to the set.
            pub fn insert_range(&mut self, range: RangeInclusive<$elem>) {
                let (first, last) = (Self::to_raw(*range.start()), Self::to_raw(*range.end()));
                self.set.insert_range(first..=last)
            }

            /// Adds all values in `values` to the set, in any order.
            pub fn insert_slice(&mut self, values: &[$elem]) {
                for &value in values {
                    self.insert(value);
                }
            }

            /// Adds all values in `values` to the set, which must be sorted.
            ///
            /// See [`Set::insert_sorted_slice`] for details.
            pub fn insert_sorted_slice(&mut self, values: &[$elem]) {
                // SAFETY: `$elem` has the same size, alignment and value
                // representation as `u32` (checked at compile time above), so
                // the slice can be reinterpreted without copying.
                let raw = unsafe {
                    std::slice::from_raw_parts(values.as_ptr() as *const u32, values.len())
                };
                self.set.insert_sorted_slice(raw)
            }

            /// Removes `value` from the set.
            pub fn remove(&mut self, value: $elem) {
                self.set.remove(Self::to_raw(value))
            }

            /// Removes all values in `range` from the set.
            pub fn remove_range(&mut self, range: RangeInclusive<$elem>) {
                let (first, last) = (Self::to_raw(*range.start()), Self::to_raw(*range.end()));
                self.set.remove_range(first..=last)
            }

            /// Removes all elements from the set.
            pub fn clear(&mut self) {
                self.set.clear()
            }

            /// Returns an iterator over the elements of the set in ascending
            /// order.
            ///
            /// Stored values that are not valid values of the element type are
            /// skipped. Use `iter()` on the dereferenced [`Set`] to get the raw
            /// values.
            pub fn iter(&self) -> impl Iterator<Item = $elem> + '_ {
                self.set.iter().filter_map(Self::from_raw_value)
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                fmt::Debug::fmt(&self.set, f)
            }
        }

        unsafe impl HarfbuzzObject for $name {
            type Raw = hb_set_t;

            unsafe fn from_raw(raw: *const hb_set_t) -> Self {
                $name {
                    set: Set::from_raw(raw),
                }
            }

            fn as_raw(&self) -> *mut Self::Raw {
                self.set.as_raw()
            }

            unsafe fn reference(&self) {
                self.set.reference()
            }

            unsafe fn dereference(&self) {
                self.set.dereference()
            }
        }
    };
}

/// A [`Set`] of Unicode codepoints.
///
/// HarfBuzz documents that some of its sets only contain codepoints, for
/// instance the [`unicode_set`](crate::subset::SubsetInput::unicode_set) of a
/// [`SubsetInput`](crate::subset::SubsetInput). A `CharSet` makes this visible
/// in the type system: it only allows inserting and removing `char`s.
///
/// There is no safe way to create a `CharSet`. You obtain one as a
/// [`Borrowed`](crate::Borrowed) or [`BorrowedMut`](crate::BorrowedMut) from the
/// object that owns it, e.g. via
/// [`SubsetInput::unicode_set_mut`](crate::subset::SubsetInput::unicode_set_mut).
///
/// A `CharSet` dereferences to the untyped [`Set`] for read-only access, so all
/// of its non-mutating methods such as [`len`](Set::len) are available.
/// Mutation is only possible through the typed methods defined here.
///
/// # Examples
///
/// ```
/// use harfbuzz_rs::subset::SubsetInput;
///
/// let mut input = SubsetInput::new().unwrap();
/// let mut unicode = input.unicode_set_mut();
/// unicode.insert('a');
/// unicode.insert_range('x'..='z');
/// unicode.insert_str("hello");
/// assert!(unicode.contains('h'));
/// assert_eq!(unicode.len(), 8);
/// ```
pub struct CharSet {
    set: Set,
}

impl CharSet {
    fn to_raw(value: char) -> u32 {
        value as u32
    }

    fn from_raw_value(raw: u32) -> Option<char> {
        char::from_u32(raw)
    }

    /// Adds the codepoints of all characters in `text` to the set.
    pub fn insert_str(&mut self, text: &str) {
        self.set.extend(text.chars().map(u32::from));
    }
}

typed_set!(CharSet, char);

/// A [`Set`] of OpenType [`Tag`]s.
///
/// HarfBuzz documents that some of its sets only contain tags, for instance the
/// [`drop_table_set`](crate::subset::SubsetInput::drop_table_set) of a
/// [`SubsetInput`](crate::subset::SubsetInput). A `TagSet` makes this visible
/// in the type system: it only allows inserting and removing `Tag`s.
///
/// Like [`CharSet`] it has no safe constructor and dereferences to [`Set`] for
/// read-only access.
///
/// # Examples
///
/// ```
/// use harfbuzz_rs::subset::SubsetInput;
/// use harfbuzz_rs::Tag;
///
/// let mut input = SubsetInput::new().unwrap();
/// let mut tables = input.drop_table_set_mut();
/// tables.insert(Tag::new('G', 'P', 'O', 'S'));
/// assert!(tables.contains(Tag::new('G', 'P', 'O', 'S')));
/// ```
///
/// Inserting a value of another type is a compile error:
///
/// ```compile_fail
/// use harfbuzz_rs::subset::SubsetInput;
///
/// let mut input = SubsetInput::new().unwrap();
/// // The set holds `Tag`s, not integers.
/// input.drop_table_set_mut().insert(42);
/// ```
pub struct TagSet {
    set: Set,
}

impl TagSet {
    fn to_raw(value: Tag) -> u32 {
        value.0
    }

    fn from_raw_value(raw: u32) -> Option<Tag> {
        Some(Tag(raw))
    }
}

typed_set!(TagSet, Tag);

#[cfg(test)]
mod tests {
    use super::*;

    fn set_of(values: &[u32]) -> Owned<Set> {
        values.iter().copied().collect()
    }

    fn elements(set: &Set) -> Vec<u32> {
        set.iter().collect()
    }

    #[test]
    fn insert_remove_contains() {
        let mut set = Set::new();
        assert!(set.is_empty());
        assert_eq!(set.len(), 0);

        set.insert(5);
        set.insert(5);
        set.insert(u32::MAX - 1);
        assert!(set.contains(5));
        assert!(!set.contains(6));
        assert_eq!(set.len(), 2);

        set.remove(5);
        assert!(!set.contains(5));
        assert_eq!(set.len(), 1);

        set.clear();
        assert!(set.is_empty());
    }

    #[test]
    fn ranges_and_min_max() {
        let mut set = Set::new();
        assert_eq!((set.min(), set.max()), (None, None));

        set.insert_range(10..=20);
        set.insert(30);
        assert_eq!((set.min(), set.max()), (Some(10), Some(30)));
        assert_eq!(set.len(), 12);

        set.remove_range(12..=18);
        assert_eq!(elements(&set), [10, 11, 19, 20, 30]);
        assert_eq!(
            set.ranges().collect::<Vec<_>>(),
            [10..=11, 19..=20, 30..=30]
        );
    }

    #[test]
    fn set_algebra() {
        let a = set_of(&[1, 2, 3, 4]);
        let b = set_of(&[3, 4, 5]);

        let mut union = a.copy();
        union.union_with(&b);
        assert_eq!(elements(&union), [1, 2, 3, 4, 5]);

        let mut intersection = a.copy();
        intersection.intersect_with(&b);
        assert_eq!(elements(&intersection), [3, 4]);

        let mut difference = a.copy();
        difference.subtract(&b);
        assert_eq!(elements(&difference), [1, 2]);

        let mut symmetric = a.copy();
        symmetric.symmetric_difference_with(&b);
        assert_eq!(elements(&symmetric), [1, 2, 5]);

        let mut assigned = Set::new();
        assigned.assign(&b);
        assert_eq!(assigned, b);
    }

    #[test]
    fn relations() {
        let small = set_of(&[3, 4]);
        let big = set_of(&[1, 2, 3, 4]);
        let other = set_of(&[7]);

        assert!(small.is_subset(&big));
        assert!(!big.is_subset(&small));
        assert!(small.intersects(&big));
        assert!(!small.intersects(&other));
    }

    #[test]
    fn invert() {
        let mut set = set_of(&[0, 1]);
        assert!(!set.is_inverted());
        set.invert();
        assert!(set.is_inverted());
        assert!(!set.contains(0));
        assert!(set.contains(2));
        set.invert();
        assert_eq!(elements(&set), [0, 1]);
    }

    #[test]
    fn copy_is_independent() {
        let original = set_of(&[1, 2]);
        let mut copy = original.copy();
        assert_eq!(*copy, *original);

        copy.insert(3);
        assert_ne!(*copy, *original);
        assert_eq!(original.len(), 2);
    }

    #[test]
    fn eq_and_hash_agree() {
        use std::collections::hash_map::DefaultHasher;
        let hash = |set: &Set| {
            let mut hasher = DefaultHasher::new();
            set.hash(&mut hasher);
            hasher.finish()
        };
        assert_eq!(hash(&set_of(&[1, 2, 3])), hash(&set_of(&[3, 2, 1])));
    }

    #[test]
    fn empty_set_is_immutable() {
        let empty = Set::empty();
        assert!(empty.is_empty());
        assert_eq!(empty.len(), 0);
        assert_eq!(empty.iter().next(), None);
    }

    #[test]
    fn insert_sorted_slice() {
        let values: Vec<u32> = (0..1000).map(|i| i * 3).collect();
        let mut set = Set::new();
        set.insert_sorted_slice(&values);
        set.insert_sorted_slice(&[]);
        assert_eq!(set.len(), 1000);
        assert_eq!(set.iter().collect::<Vec<_>>(), values);
    }

    #[test]
    fn insert_unsorted_slice_is_memory_safe() {
        let mut set = Set::new();
        set.insert_sorted_slice(&[5, 6, 1, 7]);
        // Everything before the first out-of-order value is inserted.
        assert!(set.contains(5) && set.contains(6));
        assert!(set.len() <= 4);
    }

    #[test]
    fn insert_slice_accepts_unsorted_input() {
        let mut set = Set::new();
        set.insert_slice(&[9, 1, 5, 1]);
        set.insert_slice(&[]);
        assert_eq!(set.iter().collect::<Vec<_>>(), [1, 5, 9]);
    }

    #[test]
    fn debug_output() {
        let set = set_of(&[1, 2, 3, 7]);
        assert_eq!(format!("{:?}", &*set), "{1..=3, 7..=7}");
    }

    #[test]
    fn iterate_by_reference() {
        let set = set_of(&[4, 2, 8]);
        let mut sum = 0;
        for value in &*set {
            sum += value;
        }
        assert_eq!(sum, 14);
    }
}
