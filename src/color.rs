use std::{
    fmt::Debug,
    ops::{Index, IndexMut, Not},
};

/// Stone color on the board.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Color {
    Black = 0,
    White = 1,
}

impl TryFrom<usize> for Color {
    type Error = ();

    fn try_from(value: usize) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Color::Black),
            1 => Ok(Color::White),
            _ => Err(()),
        }
    }
}

impl From<Color> for usize {
    fn from(value: Color) -> Self {
        match value {
            Color::Black => 0,
            Color::White => 1,
        }
    }
}

impl Not for Color {
    type Output = Color;

    fn not(self) -> Self::Output {
        match self {
            Color::Black => Color::White,
            Color::White => Color::Black,
        }
    }
}

/// Collection indexed by `Color`.
#[derive(Default, Clone, PartialEq, Eq)]
pub struct ColorMap<V> {
    values: [V; 2],
}

impl<T> ColorMap<T> {
    /// Returns a `ColorMap` with function `f` applied to each element.
    /// Black is processed first, then White.
    pub fn map<U>(self, f: impl FnMut(T) -> U) -> ColorMap<U> {
        ColorMap {
            values: self.values.map(f),
        }
    }

    /// Returns a `ColorMap` with function `f` applied to each element.
    /// Black is processed first, then White.
    pub fn map_ref<U>(&self, f: impl FnMut(&T) -> U) -> ColorMap<U> {
        ColorMap {
            values: self.values.each_ref().map(f),
        }
    }

    /// Borrows each held element and returns colormap of references.
    pub fn as_ref(&self) -> ColorMap<&T> {
        ColorMap {
            values: self.values.each_ref(),
        }
    }

    /// Convert the colormap to raw array.
    pub fn into_array(self) -> [T; 2] {
        self.values
    }

    /// Returns the content from perspective of given color.
    /// Complementary to `ColorMap::from_perspective`.
    pub fn into_perspective(self, color: Color) -> (T, T) {
        let [b, w] = self.values;
        match color {
            Color::Black => (b, w),
            Color::White => (w, b),
        }
    }

    /// Constructs a ColorMap from the perspective of given color.
    /// Complementary to `ColorMap::to_perspective`.
    pub fn from_perspective(color: Color, own: T, other: T) -> ColorMap<T> {
        match color {
            Color::Black => ColorMap {
                values: [own, other],
            },
            Color::White => ColorMap {
                values: [other, own],
            },
        }
    }
}

impl<T: IntoIterator> ColorMap<T> {
    pub fn zip(self) -> impl Iterator<Item = ColorMap<T::Item>> {
        let [b, w] = self.values;
        b.into_iter().zip(w).map(|(b, w)| [b, w].into())
    }
}

impl<T: Debug> Debug for ColorMap<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_map().entries(self.as_ref()).finish()
    }
}

impl<T> From<[T; 2]> for ColorMap<T> {
    fn from(values: [T; 2]) -> Self {
        Self { values }
    }
}

impl<T> From<ColorMap<T>> for [T; 2] {
    fn from(value: ColorMap<T>) -> Self {
        value.into_array()
    }
}

impl<T> Index<Color> for ColorMap<T> {
    type Output = T;

    fn index(&self, index: Color) -> &Self::Output {
        &self.values[usize::from(index)]
    }
}

impl<T> IndexMut<Color> for ColorMap<T> {
    fn index_mut(&mut self, index: Color) -> &mut Self::Output {
        &mut self.values[usize::from(index)]
    }
}

impl<T> IntoIterator for ColorMap<T> {
    type Item = (Color, T);
    type IntoIter = std::array::IntoIter<(Color, T), 2>;

    fn into_iter(self) -> Self::IntoIter {
        let [b, w] = self.values;
        [(Color::Black, b), (Color::White, w)].into_iter()
    }
}

#[cfg(test)]
impl proptest::arbitrary::Arbitrary for Color {
    type Parameters = ();
    type Strategy = proptest::strategy::BoxedStrategy<Color>;

    fn arbitrary_with(_: Self::Parameters) -> Self::Strategy {
        use proptest::prelude::Strategy;

        (0usize..=1usize)
            .prop_map(|v| v.try_into().unwrap())
            .boxed()
    }
}

#[cfg(test)]
impl<T> proptest::arbitrary::Arbitrary for ColorMap<T>
where
    T: proptest::arbitrary::Arbitrary + 'static,
{
    type Parameters = T::Parameters;
    type Strategy = proptest::strategy::BoxedStrategy<ColorMap<T>>;

    fn arbitrary_with(args: Self::Parameters) -> Self::Strategy {
        use proptest::prelude::Strategy;

        proptest::array::uniform2(T::arbitrary_with(args))
            .prop_map_into()
            .boxed()
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use proptest::property_test;

    mod color {
        use super::*;

        #[property_test]
        fn usize_roundtrip(c: Color) {
            assert_eq!(Color::try_from(usize::from(c)).unwrap(), c);
        }

        #[property_test]
        fn wrong_usize(#[strategy = 2usize..] v: usize) {
            assert!(Color::try_from(v).is_err());
        }

        #[property_test]
        fn negation_usize(c: Color) {
            assert_eq!(usize::from(!c), 1 - usize::from(c));
        }
    }

    mod color_map {
        use super::*;

        #[test]
        fn from_array_order() {
            let m = ColorMap::from([1, 2]);

            assert_eq!(m[Color::Black], 1);
            assert_eq!(m[Color::White], 2);
        }

        #[test]
        fn from_array_round_trip() {
            let array = [1, 2];
            let m = ColorMap::from(array);
            assert_eq!(m.into_array(), array);
        }

        #[test]
        fn index_mut() {
            let mut colormap = ColorMap::from([1, 2]);

            colormap[Color::Black] = 3;
            assert_eq!(colormap.clone().into_array(), [3, 2]);

            colormap[Color::White] = 4;
            assert_eq!(colormap.into_array(), [3, 4]);
        }

        #[test]
        fn iterator() {
            let m = ColorMap::from([1, 2]);
            let v: Vec<_> = m.clone().into_iter().collect();

            assert_eq!(v, vec![(Color::Black, 1), (Color::White, 2)]);
        }

        #[test]
        fn debug_format() {
            assert_eq!(
                format!("{:?}", ColorMap::from([1, 2])),
                "{Black: 1, White: 2}"
            );
        }

        #[property_test]
        fn to_perspective_matches_color(c: Color) {
            let colormap = ColorMap::from([1, 2]);
            assert_eq!(
                colormap.clone().into_perspective(c),
                (colormap[c], colormap[!c])
            );
        }

        #[test]
        fn to_perspective_symmetry() {
            assert_eq!(
                ColorMap::from_perspective(Color::Black, 1, 2),
                ColorMap::from_perspective(Color::White, 2, 1)
            );
        }

        #[property_test]
        fn perspective_roundtrip(c: Color) {
            assert_eq!(
                ColorMap::from_perspective(c, 1, 2).into_perspective(c),
                (1, 2)
            );
        }

        #[test]
        fn map() {
            let colormap = ColorMap::from([1, 2]);
            assert_eq!(colormap.map(|x| 10 * x).into_array(), [10, 20]);
        }

        #[test]
        fn map_ref() {
            let colormap = ColorMap::from([1, 2]);
            assert_eq!(colormap.map_ref(|x| 10 * x).into_array(), [10, 20]);
        }

        #[test]
        fn map_order() {
            let colormap = ColorMap::from([9, 8]);
            let mut counter = 0;
            assert_eq!(
                colormap
                    .map(move |_| {
                        counter += 1;
                        counter
                    })
                    .into_array(),
                [1, 2]
            );
        }

        #[test]
        fn zip() {
            let colormap = ColorMap::from([0..3, 10..13]);

            assert_eq!(
                colormap.zip().collect::<Vec<_>>(),
                vec![
                    ColorMap::from([0, 10]),
                    ColorMap::from([1, 11]),
                    ColorMap::from([2, 12])
                ]
            );
        }
    }
}
