use crate::{
    CoreError, TickArrayFacade, TickFacade, INVALID_TICK_ARRAY_SEQUENCE, INVALID_TICK_INDEX,
    MAX_TICK_INDEX, MIN_TICK_INDEX, TICK_ARRAY_NOT_EVENLY_SPACED, TICK_ARRAY_SIZE,
    TICK_INDEX_OUT_OF_BOUNDS, TICK_SEQUENCE_EMPTY,
};

use super::{
    get_initializable_tick_index, get_next_initializable_tick_index,
    get_prev_initializable_tick_index,
};

trait TickArrayStorage {
    fn len(&self) -> usize;
    fn array(&self, index: usize) -> Option<&TickArrayFacade>;
}

impl<const SIZE: usize> TickArrayStorage for [Option<TickArrayFacade>; SIZE] {
    fn len(&self) -> usize {
        SIZE
    }
    fn array(&self, index: usize) -> Option<&TickArrayFacade> {
        self.get(index).and_then(Option::as_ref)
    }
}

#[cfg(not(feature = "wasm"))]
impl<const SIZE: usize> TickArrayStorage for [Option<&TickArrayFacade>; SIZE] {
    fn len(&self) -> usize {
        SIZE
    }
    fn array(&self, index: usize) -> Option<&TickArrayFacade> {
        self.get(index).copied().flatten()
    }
}

fn array_start(array: Option<&TickArrayFacade>) -> i32 {
    array.map_or(i32::MAX, |array| array.start_tick_index)
}

fn validate_sequence(arrays: &impl TickArrayStorage, spacing: u16) -> Result<(), CoreError> {
    if arrays.len() == 0 || arrays.array(0).is_none() {
        return Err(TICK_SEQUENCE_EMPTY);
    }
    let required = TICK_ARRAY_SIZE as i32 * spacing as i32;
    for index in 0..arrays.len() - 1 {
        let current = array_start(arrays.array(index));
        let next = array_start(arrays.array(index + 1));
        if next != i32::MAX && next - current != required {
            return Err(TICK_ARRAY_NOT_EVENLY_SPACED);
        }
    }
    Ok(())
}

fn sequence_start_index(arrays: &impl TickArrayStorage) -> i32 {
    array_start(arrays.array(0)).max(MIN_TICK_INDEX)
}

fn sequence_end_index(arrays: &impl TickArrayStorage, spacing: u16) -> i32 {
    let mut last = sequence_start_index(arrays);
    for index in 0..arrays.len() {
        let start = array_start(arrays.array(index));
        if start != i32::MAX {
            last = start;
        }
    }
    (last + TICK_ARRAY_SIZE as i32 * spacing as i32 - 1).min(MAX_TICK_INDEX)
}

fn sequence_tick(
    arrays: &impl TickArrayStorage,
    spacing: u16,
    tick_index: i32,
) -> Result<&TickFacade, CoreError> {
    if tick_index < sequence_start_index(arrays) || tick_index > sequence_end_index(arrays, spacing)
    {
        return Err(TICK_INDEX_OUT_OF_BOUNDS);
    }
    if tick_index % spacing as i32 != 0 {
        return Err(INVALID_TICK_INDEX);
    }
    let first = array_start(arrays.array(0));
    let array_index = ((tick_index - first) / (TICK_ARRAY_SIZE as i32 * spacing as i32)) as usize;
    let array = arrays.array(array_index).ok_or(TICK_INDEX_OUT_OF_BOUNDS)?;
    let offset = (tick_index - array.start_tick_index) / spacing as i32;
    Ok(&array.ticks[offset as usize])
}

fn sequence_next_initialized_tick(
    arrays: &impl TickArrayStorage,
    spacing: u16,
    tick_index: i32,
) -> Result<(Option<&TickFacade>, i32), CoreError> {
    let end = sequence_end_index(arrays, spacing);
    if tick_index >= end {
        return Err(INVALID_TICK_ARRAY_SEQUENCE);
    }
    let mut next = tick_index;
    loop {
        next = get_next_initializable_tick_index(next, spacing);
        if next > end {
            return Ok((None, end));
        }
        let tick = sequence_tick(arrays, spacing, next)?;
        if tick.initialized {
            return Ok((Some(tick), next));
        }
    }
}

fn sequence_prev_initialized_tick(
    arrays: &impl TickArrayStorage,
    spacing: u16,
    tick_index: i32,
) -> Result<(Option<&TickFacade>, i32), CoreError> {
    let start = sequence_start_index(arrays);
    if tick_index < start {
        return Err(INVALID_TICK_ARRAY_SEQUENCE);
    }
    let mut previous = get_initializable_tick_index(tick_index, spacing, Some(false));
    loop {
        if previous < start {
            return Ok((None, start));
        }
        let tick = sequence_tick(arrays, spacing, previous)?;
        if tick.initialized {
            return Ok((Some(tick), previous));
        }
        previous = get_prev_initializable_tick_index(previous, spacing);
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TickArraySequence<const SIZE: usize> {
    pub tick_arrays: [Option<TickArrayFacade>; SIZE],
    pub tick_spacing: u16,
}

impl<const SIZE: usize> TickArraySequence<SIZE> {
    pub fn new(
        mut tick_arrays: [Option<TickArrayFacade>; SIZE],
        tick_spacing: u16,
    ) -> Result<Self, CoreError> {
        tick_arrays.sort_by_key(|array| array_start(array.as_ref()));
        validate_sequence(&tick_arrays, tick_spacing)?;
        Ok(Self {
            tick_arrays,
            tick_spacing,
        })
    }
    pub fn start_index(&self) -> i32 {
        sequence_start_index(&self.tick_arrays)
    }
    pub fn end_index(&self) -> i32 {
        sequence_end_index(&self.tick_arrays, self.tick_spacing)
    }
    pub fn tick(&self, tick_index: i32) -> Result<&TickFacade, CoreError> {
        sequence_tick(&self.tick_arrays, self.tick_spacing, tick_index)
    }
    pub fn next_initialized_tick(
        &self,
        tick_index: i32,
    ) -> Result<(Option<&TickFacade>, i32), CoreError> {
        sequence_next_initialized_tick(&self.tick_arrays, self.tick_spacing, tick_index)
    }
    pub fn prev_initialized_tick(
        &self,
        tick_index: i32,
    ) -> Result<(Option<&TickFacade>, i32), CoreError> {
        sequence_prev_initialized_tick(&self.tick_arrays, self.tick_spacing, tick_index)
    }
}
#[cfg(not(feature = "wasm"))]
#[derive(Clone, Copy, Debug)]
pub struct BorrowedTickArraySequence<'a, const SIZE: usize> {
    pub tick_arrays: [Option<&'a TickArrayFacade>; SIZE],
    pub tick_spacing: u16,
}
#[cfg(not(feature = "wasm"))]
impl<'a, const SIZE: usize> BorrowedTickArraySequence<'a, SIZE> {
    pub fn new(
        mut tick_arrays: [Option<&'a TickArrayFacade>; SIZE],
        tick_spacing: u16,
    ) -> Result<Self, CoreError> {
        tick_arrays.sort_by_key(|array| array_start(*array));
        validate_sequence(&tick_arrays, tick_spacing)?;
        Ok(Self {
            tick_arrays,
            tick_spacing,
        })
    }
    pub fn start_index(&self) -> i32 {
        sequence_start_index(&self.tick_arrays)
    }
    pub fn end_index(&self) -> i32 {
        sequence_end_index(&self.tick_arrays, self.tick_spacing)
    }
    pub fn tick(&self, tick_index: i32) -> Result<&TickFacade, CoreError> {
        sequence_tick(&self.tick_arrays, self.tick_spacing, tick_index)
    }
    pub fn next_initialized_tick(
        &self,
        tick_index: i32,
    ) -> Result<(Option<&TickFacade>, i32), CoreError> {
        sequence_next_initialized_tick(&self.tick_arrays, self.tick_spacing, tick_index)
    }
    pub fn prev_initialized_tick(
        &self,
        tick_index: i32,
    ) -> Result<(Option<&TickFacade>, i32), CoreError> {
        sequence_prev_initialized_tick(&self.tick_arrays, self.tick_spacing, tick_index)
    }
}

#[cfg(all(test, not(feature = "wasm")))]
mod tests {
    use super::*;
    use crate::get_tick_array_start_tick_index;

    fn test_tick(initialized: bool, liquidity_net: i128) -> TickFacade {
        TickFacade {
            initialized,
            liquidity_net,
            ..TickFacade::default()
        }
    }

    fn test_ticks_initialized() -> [TickFacade; TICK_ARRAY_SIZE] {
        (0..TICK_ARRAY_SIZE)
            .map(|x| test_tick(true, x as i128))
            .collect::<Vec<TickFacade>>()
            .try_into()
            .unwrap()
    }

    fn test_ticks_uninitialized() -> [TickFacade; TICK_ARRAY_SIZE] {
        (0..TICK_ARRAY_SIZE)
            .map(|_| test_tick(false, 0))
            .collect::<Vec<TickFacade>>()
            .try_into()
            .unwrap()
    }

    fn test_ticks_alternating_initialized() -> [TickFacade; TICK_ARRAY_SIZE] {
        (0..TICK_ARRAY_SIZE)
            .map(|x| {
                let initialized = x & 1 == 1;
                test_tick(initialized, if initialized { x as i128 } else { 0 })
            })
            .collect::<Vec<TickFacade>>()
            .try_into()
            .unwrap()
    }

    fn test_sequence_with_one_tick_array(
        tick_spacing: u16,
        ticks: [TickFacade; TICK_ARRAY_SIZE],
        start_tick_index: i32,
    ) -> TickArraySequence<5> {
        let one = TickArrayFacade {
            start_tick_index,
            ticks,
        };
        TickArraySequence::new([Some(one), None, None, None, None], tick_spacing).unwrap()
    }

    fn test_sequence(
        tick_spacing: u16,
        ticks: [TickFacade; TICK_ARRAY_SIZE],
    ) -> TickArraySequence<5> {
        let one = TickArrayFacade {
            start_tick_index: -(TICK_ARRAY_SIZE as i32 * tick_spacing as i32),
            ticks,
        };
        let two = TickArrayFacade {
            start_tick_index: 0,
            ticks,
        };
        let three = TickArrayFacade {
            start_tick_index: TICK_ARRAY_SIZE as i32 * tick_spacing as i32,
            ticks,
        };
        TickArraySequence::new(
            [Some(one), Some(two), Some(three), None, None],
            tick_spacing,
        )
        .unwrap()
    }

    #[test]
    fn test_tick_array_start_index() {
        let sequence = test_sequence(16, test_ticks_alternating_initialized());
        assert_eq!(sequence.start_index(), -1408);
    }

    #[test]
    fn test_tick_array_end_index() {
        let sequence = test_sequence(16, test_ticks_alternating_initialized());
        assert_eq!(sequence.end_index(), 2815);
    }

    #[test]
    fn test_get_tick() {
        let sequence = test_sequence(16, test_ticks_alternating_initialized());
        assert_eq!(sequence.tick(-1408).map(|x| x.liquidity_net), Ok(0));
        assert_eq!(sequence.tick(-16).map(|x| x.liquidity_net), Ok(87));
        assert_eq!(sequence.tick(0).map(|x| x.liquidity_net), Ok(0));
        assert_eq!(sequence.tick(16).map(|x| x.liquidity_net), Ok(1));
        assert_eq!(sequence.tick(1408).map(|x| x.liquidity_net), Ok(0));
        assert_eq!(sequence.tick(1424).map(|x| x.liquidity_net), Ok(1));
    }

    #[test]
    fn test_get_tick_large_tick_spacing() {
        let sequence: TickArraySequence<5> =
            test_sequence(32896, test_ticks_alternating_initialized());
        assert_eq!(sequence.tick(-427648).map(|x| x.liquidity_net), Ok(75));
        assert_eq!(sequence.tick(0).map(|x| x.liquidity_net), Ok(0));
        assert_eq!(sequence.tick(427648).map(|x| x.liquidity_net), Ok(13));
    }

    #[test]
    fn test_get_tick_errors() {
        let sequence = test_sequence(16, test_ticks_alternating_initialized());

        let out_out_bounds_lower = sequence.tick(-1409);
        assert!(matches!(
            out_out_bounds_lower,
            Err(TICK_INDEX_OUT_OF_BOUNDS)
        ));

        let out_of_bounds_upper = sequence.tick(2817);
        assert!(matches!(out_of_bounds_upper, Err(TICK_INDEX_OUT_OF_BOUNDS)));

        let invalid_tick_index = sequence.tick(1);
        assert!(matches!(invalid_tick_index, Err(INVALID_TICK_INDEX)));

        let invalid_negative_tick_index = sequence.tick(-1);
        assert!(matches!(
            invalid_negative_tick_index,
            Err(INVALID_TICK_INDEX)
        ));
    }

    #[test]
    fn test_get_next_initializable_tick_index() {
        let sequence = test_sequence(16, test_ticks_alternating_initialized());
        let pair = sequence.next_initialized_tick(0);
        assert_eq!(pair.map(|x| x.1), Ok(16));
        assert_eq!(pair.map(|x| x.0.map(|x| x.liquidity_net)), Ok(Some(1)));
    }

    #[test]
    fn test_get_next_initializable_tick_index_off_spacing() {
        let sequence = test_sequence(16, test_ticks_alternating_initialized());
        let pair = sequence.next_initialized_tick(-17);
        assert_eq!(pair.map(|x| x.1), Ok(-16));
        assert_eq!(pair.map(|x| x.0.map(|x| x.liquidity_net)), Ok(Some(87)));
    }

    #[test]
    fn test_get_next_initializable_tick_cross_array() {
        let sequence = test_sequence(16, test_ticks_alternating_initialized());
        let pair = sequence.next_initialized_tick(1392);
        assert_eq!(pair.map(|x| x.1), Ok(1424));
        assert_eq!(pair.map(|x| x.0.map(|x| x.liquidity_net)), Ok(Some(1)));
    }

    #[test]
    fn test_get_next_initializable_tick_skip_uninitialized() {
        let sequence = test_sequence(16, test_ticks_alternating_initialized());
        let pair = sequence.next_initialized_tick(-1);
        assert_eq!(pair.map(|x| x.1), Ok(16));
        assert_eq!(pair.map(|x| x.0.map(|x| x.liquidity_net)), Ok(Some(1)));
    }

    #[test]
    fn test_get_next_initializable_tick_invalid_tick_array_sequence() {
        let sequence = test_sequence(16, test_ticks_alternating_initialized());
        let pair_2813 = sequence.next_initialized_tick(2813);
        let pair_2814 = sequence.next_initialized_tick(2814);
        let pair_2815 = sequence.next_initialized_tick(2815);
        let pair_2816 = sequence.next_initialized_tick(2816);
        assert_eq!(pair_2813, Ok((None, 2815)));
        assert_eq!(pair_2814, Ok((None, 2815)));
        assert_eq!(pair_2815, Err(INVALID_TICK_ARRAY_SEQUENCE));
        assert_eq!(pair_2816, Err(INVALID_TICK_ARRAY_SEQUENCE));
    }

    #[test]
    fn test_get_next_initializable_tick_with_last_initializable_tick_initialized() {
        let sequence = test_sequence(16, test_ticks_initialized());
        let pair_2799 = sequence.next_initialized_tick(2799);
        let pair_2800 = sequence.next_initialized_tick(2800);
        assert_eq!(pair_2799, Ok((Some(&test_tick(true, 87)), 2800)));
        assert_eq!(pair_2800, Ok((None, 2815)));
    }

    #[test]
    fn test_get_next_initializable_tick_with_last_initializable_tick_uninitialized() {
        let sequence = test_sequence(16, test_ticks_uninitialized());
        let pair_2799 = sequence.next_initialized_tick(2799);
        assert_eq!(pair_2799, Ok((None, 2815)));
    }

    #[test]
    fn test_get_next_initializable_tick_in_end_tick_array_with_uninitialized_ticks_ts_16() {
        let tick_spacing = 16;
        let start_tick_index = get_tick_array_start_tick_index(MAX_TICK_INDEX, tick_spacing);
        let sequence = test_sequence_with_one_tick_array(
            tick_spacing,
            test_ticks_uninitialized(),
            start_tick_index,
        );
        let pair = sequence.next_initialized_tick(start_tick_index);
        assert_eq!(pair, Ok((None, MAX_TICK_INDEX)));
    }

    #[test]
    fn test_get_next_initializable_tick_in_end_tick_array_with_uninitialized_ticks_ts_1() {
        let tick_spacing = 1;
        let start_tick_index = get_tick_array_start_tick_index(MAX_TICK_INDEX, tick_spacing);
        let sequence = test_sequence_with_one_tick_array(
            tick_spacing,
            test_ticks_uninitialized(),
            start_tick_index,
        );
        let pair = sequence.next_initialized_tick(start_tick_index);
        assert_eq!(pair, Ok((None, MAX_TICK_INDEX)));
    }

    #[test]
    fn test_get_next_initializable_tick_in_end_tick_array_with_initialized_ticks_ts_1() {
        let tick_spacing = 1;
        let start_tick_index = get_tick_array_start_tick_index(MAX_TICK_INDEX, tick_spacing);
        let sequence = test_sequence_with_one_tick_array(
            tick_spacing,
            test_ticks_initialized(),
            start_tick_index,
        );
        let pair = sequence.next_initialized_tick(MAX_TICK_INDEX - 1);
        assert_eq!(pair, Ok((Some(&test_tick(true, 28)), MAX_TICK_INDEX)));
    }

    #[test]
    fn test_get_prev_initializable_tick_index() {
        let sequence = test_sequence(16, test_ticks_alternating_initialized());
        let pair = sequence.prev_initialized_tick(32);
        assert_eq!(pair.map(|x| x.1), Ok(16));
        assert_eq!(pair.map(|x| x.0.map(|x| x.liquidity_net)), Ok(Some(1)));
    }

    #[test]
    fn test_get_prev_initializable_tick_index_off_spacing() {
        let sequence = test_sequence(16, test_ticks_alternating_initialized());
        let pair = sequence.prev_initialized_tick(-1);
        assert_eq!(pair.map(|x| x.1), Ok(-16));
        assert_eq!(pair.map(|x| x.0.map(|x| x.liquidity_net)), Ok(Some(87)));
    }

    #[test]
    fn test_get_prev_initializable_tick_skip_uninitialized() {
        let sequence = test_sequence(16, test_ticks_alternating_initialized());
        let pair = sequence.prev_initialized_tick(33);
        assert_eq!(pair.map(|x| x.1), Ok(16));
        assert_eq!(pair.map(|x| x.0.map(|x| x.liquidity_net)), Ok(Some(1)));
    }

    #[test]
    fn test_get_prev_initializable_tick_cross_array() {
        let sequence = test_sequence(16, test_ticks_alternating_initialized());
        let pair = sequence.prev_initialized_tick(1408);
        assert_eq!(pair.map(|x| x.1), Ok(1392));
        assert_eq!(pair.map(|x| x.0.map(|x| x.liquidity_net)), Ok(Some(87)));
    }

    #[test]
    fn test_get_prev_initialized_tick_invalid_tick_array_sequence() {
        let sequence = test_sequence(16, test_ticks_alternating_initialized());
        let pair_1407 = sequence.prev_initialized_tick(-1407);
        let pair_1408 = sequence.prev_initialized_tick(-1408);
        let pair_1409 = sequence.prev_initialized_tick(-1409);
        let pair_1410 = sequence.prev_initialized_tick(-1410);
        assert!(matches!(pair_1407, Ok((None, -1408))));
        assert!(matches!(pair_1408, Ok((None, -1408))));
        assert!(matches!(pair_1409, Err(INVALID_TICK_ARRAY_SEQUENCE)));
        assert!(matches!(pair_1410, Err(INVALID_TICK_ARRAY_SEQUENCE)));
    }

    #[test]
    fn test_get_prev_initializable_tick_with_first_initializable_tick_initialized() {
        let sequence = test_sequence(16, test_ticks_initialized());
        let pair = sequence.prev_initialized_tick(-1408);
        assert_eq!(pair, Ok((Some(&test_tick(true, 0)), -1408)));
    }

    #[test]
    fn test_get_prev_initializable_tick_with_first_initializable_tick_uninitialized() {
        let sequence = test_sequence(16, test_ticks_uninitialized());
        let pair = sequence.prev_initialized_tick(-1408);
        assert_eq!(pair, Ok((None, -1408)));
    }

    #[test]
    fn test_get_prev_initializable_tick_in_first_tick_array_with_uninitialized_ticks_ts_16() {
        let tick_spacing = 16;
        let start_tick_index = get_tick_array_start_tick_index(MIN_TICK_INDEX, tick_spacing);
        let sequence = test_sequence_with_one_tick_array(
            tick_spacing,
            test_ticks_uninitialized(),
            start_tick_index,
        );
        let pair = sequence.prev_initialized_tick(MIN_TICK_INDEX + tick_spacing as i32);
        assert_eq!(pair, Ok((None, MIN_TICK_INDEX)));
    }

    #[test]
    fn test_get_prev_initializable_tick_in_first_tick_array_with_uninitialized_ticks_ts_1() {
        let tick_spacing = 1;
        let start_tick_index = get_tick_array_start_tick_index(MIN_TICK_INDEX, tick_spacing);
        let sequence = test_sequence_with_one_tick_array(
            tick_spacing,
            test_ticks_uninitialized(),
            start_tick_index,
        );
        let pair = sequence.prev_initialized_tick(MIN_TICK_INDEX + tick_spacing as i32);
        assert_eq!(pair, Ok((None, MIN_TICK_INDEX)));
    }

    #[test]
    fn test_get_prev_initializable_tick_in_first_tick_array_with_initialized_ticks_ts_1() {
        let tick_spacing = 1;
        let start_tick_index = get_tick_array_start_tick_index(MIN_TICK_INDEX, tick_spacing);
        let sequence = test_sequence_with_one_tick_array(
            tick_spacing,
            test_ticks_initialized(),
            start_tick_index,
        );
        let pair = sequence.prev_initialized_tick(MIN_TICK_INDEX);
        assert_eq!(pair, Ok((Some(&test_tick(true, 60)), MIN_TICK_INDEX)));
    }
    #[test]
    fn borrowed_sequence_matches_owned_across_storage_shapes() {
        fn compare<const SIZE: usize>(arrays: [Option<TickArrayFacade>; SIZE], spacing: u16) {
            let owned_arrays = arrays;
            let refs: [Option<&TickArrayFacade>; SIZE] =
                std::array::from_fn(|index| arrays[index].as_ref());
            let owned = TickArraySequence::new(owned_arrays, spacing);
            let borrowed = BorrowedTickArraySequence::new(refs, spacing);
            assert_eq!(owned.as_ref().err(), borrowed.as_ref().err());
            let (Ok(owned), Ok(borrowed)) = (owned, borrowed) else {
                return;
            };
            assert_eq!(owned.start_index(), borrowed.start_index());
            assert_eq!(owned.end_index(), borrowed.end_index());
            let probes = [
                owned.start_index().saturating_sub(1),
                owned.start_index(),
                owned.start_index().saturating_add(spacing as i32),
                0,
                owned.end_index().saturating_sub(spacing as i32),
                owned.end_index(),
                owned.end_index().saturating_add(1),
            ];
            for probe in probes {
                assert_eq!(owned.tick(probe), borrowed.tick(probe));
                assert_eq!(
                    owned.next_initialized_tick(probe),
                    borrowed.next_initialized_tick(probe)
                );
                assert_eq!(
                    owned.prev_initialized_tick(probe),
                    borrowed.prev_initialized_tick(probe)
                );
            }
        }

        for spacing in [1, 2, 8, 64] {
            let width = TICK_ARRAY_SIZE as i32 * spacing as i32;
            let make = |start, phase| {
                let mut ticks = test_ticks_uninitialized();
                for (index, tick) in ticks.iter_mut().enumerate() {
                    if index % 3 == phase {
                        *tick = test_tick(true, start as i128 + index as i128);
                    }
                }
                TickArrayFacade {
                    start_tick_index: start,
                    ticks,
                }
            };
            compare([Some(make(0, 0))], spacing);
            compare(
                [
                    Some(make(width, 1)),
                    Some(make(-width, 2)),
                    Some(make(0, 0)),
                ],
                spacing,
            );
            compare(
                [
                    Some(make(width * 2, 2)),
                    Some(make(-width * 3, 0)),
                    Some(make(width, 1)),
                    Some(make(-width, 2)),
                    Some(make(0, 0)),
                    Some(make(-width * 2, 1)),
                ],
                spacing,
            );
            compare([Some(make(0, 0)), Some(make(width * 2, 1)), None], spacing);
            compare([None::<TickArrayFacade>], spacing);
        }
    }
}
