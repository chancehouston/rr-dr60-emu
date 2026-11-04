//! The re-render of a processing record (spec 027): one recipe, public, in the core.
//!
//! A [`Plan`] is read from a processing record (spec 020) and may be edited before the run (a
//! **Rebuild**). [`ReRender::new`] builds the engine at the record's host rate, settings, seed
//! and counter, restores the component slot and its store, and fills the memory to the record's
//! remaining capacity; the host then feeds the kept 24-bit microphone audio ([`ReRender::feed`])
//! in blocks of any length and calls [`ReRender::finish`]. The result is the rebuilt item's
//! digest, whether it matches the original's, the rebuilt stored form and a new processing
//! record written from the session's own run.
//!
//! The re-render describes how a recording was processed; it does not vouch for what the input
//! contains. It is offline work: [`ReRender::new`] allocates (like
//! [`Pipeline::new`](crate::Pipeline::new)); `feed`, `meters` and `finish` allocate nothing.

use alloc::vec;
use alloc::vec::Vec;

use crate::changes::{
    ChangeEntry, MAX_CHANGES_PER_CALL, STREAM_RECORD, Stream, record_value, slot_of,
};
use crate::components::{self, ComponentId, InsertionPointId::BeforeLimiter};
use crate::record::{self, Origin, RecordReadout, RunData, StopFields};
use crate::{Error, Meters, Pipeline, Settings, StopResult, VasEvent};

/// The host samples one engine call takes at most (plan R-03): the scratch buffers' size.
pub const CHUNK: usize = 4096; // engineering target (027 plan R-03)

/// The cap on a collected run's events (spec 027 D5, the owner's answer): 16 Mi.
pub const MAX_RUN_EVENTS: u64 = 16 * 1024 * 1024; // engineering target (027 D5, the owner's answer)

/// The schedule's capacity per stream (spec 015).
const SCHEDULE: u32 = crate::changes::SCHEDULE_CAPACITY as u32; // engineering target (015: the schedule's capacity)

/// What a re-render runs: a record read, editable before [`ReRender::new`] (spec 027 D4).
#[derive(Clone, Debug)]
pub struct Plan {
    readout: RecordReadout,
    settings: Settings,
    component: ComponentId,
    store: [f32; components::COMPONENT_PARAMETER_BUDGET],
    rows: Vec<ChangeEntry>,
    counter: u64,
    remaining: u64,
    collect_run: bool,
    original: Snapshot,
    #[cfg(feature = "__test-hooks")]
    mutation: crate::__test_hooks::RerenderMutation,
}

#[derive(Clone, Debug)]
struct Snapshot {
    settings: Settings,
    component: ComponentId,
    store: [f32; components::COMPONENT_PARAMETER_BUDGET],
    rows: Vec<ChangeEntry>,
    counter: u64,
    remaining: u64,
}

impl Plan {
    /// Reads `bytes` (spec 020's reader and its refusals) into a faithful plan.
    ///
    /// # Errors
    ///
    /// The reader's errors; [`Error::InconsistentRun`] for a change row on any stream but the
    /// record stream.
    pub fn from_record(bytes: &[u8]) -> Result<Plan, Error> {
        let mut r = RecordReadout::default();
        record::read(bytes, &mut r)?;
        let mut rows = vec![ChangeEntry::default(); r.change_count as usize];
        let n = record::record_changes(bytes, 0, &mut rows);
        rows.truncate(n);
        if rows.iter().any(|c| c.stream != STREAM_RECORD) {
            return Err(Error::InconsistentRun {
                reason: "a change row on another stream than the record stream",
            });
        }
        // Every row validated as an edit is (coach post-implementation 11), so a doctored row
        // is refused here, never mid-feed.
        for c in &rows {
            let slot = slot_of(c.control).ok_or(Error::InconsistentRun {
                reason: "a change row of a control that is not a record-side value",
            })?;
            record_value(slot, c.after)?;
        }
        let original = Snapshot {
            settings: r.settings,
            component: r.component,
            store: r.store,
            rows: rows.clone(),
            counter: r.recording_counter,
            remaining: r.remaining_samples_at_start,
        };
        Ok(Plan {
            settings: r.settings,
            component: r.component,
            store: r.store,
            counter: r.recording_counter,
            remaining: r.remaining_samples_at_start,
            rows,
            readout: r,
            collect_run: true,
            original,
            #[cfg(feature = "__test-hooks")]
            mutation: Default::default(),
        })
    }

    /// The record's read-out (its identity, flags and run totals).
    pub fn readout(&self) -> &RecordReadout {
        &self.readout
    }
    /// The read-out's flags (`record::flags`).
    pub fn flags(&self) -> u32 {
        self.readout.flags
    }
    /// The settings the run starts with.
    pub fn settings(&self) -> &Settings {
        &self.settings
    }
    /// The slot's component.
    pub fn component(&self) -> ComponentId {
        self.component
    }
    /// Store value `index` (the global store index).
    pub fn parameter(&self, index: u32) -> Option<f32> {
        self.store.get(index as usize).copied()
    }
    /// The change rows, sorted by position.
    pub fn rows(&self) -> &[ChangeEntry] {
        &self.rows
    }
    /// The seed.
    pub fn seed(&self) -> u64 {
        self.settings.seed
    }
    /// The recording counter the hiss stream is keyed with.
    pub fn counter(&self) -> u64 {
        self.counter
    }
    /// The memory's remaining capacity at the start, in device samples.
    pub fn remaining_samples(&self) -> u64 {
        self.remaining
    }
    /// Whether the session collects the run (events and change entries) for a new record.
    pub fn collect_run(&self) -> bool {
        self.collect_run
    }

    /// Whether the plan's values differ from the record's: a field compare (floats by bits), not
    /// whether an edit call was made. A −0.0 written back as 0.0 reads as edited.
    pub fn edited(&self) -> bool {
        let o = &self.original;
        let settings_differ = (0..crate::controls::CONTROL_COUNT as u32).any(|i| {
            match (
                crate::control_map::settings_value(&self.settings, i),
                crate::control_map::settings_value(&o.settings, i),
            ) {
                (Ok(a), Ok(b)) => reading_bits(a) != reading_bits(b),
                _ => false,
            }
        });
        settings_differ
            || self.component != o.component
            || self
                .store
                .iter()
                .zip(&o.store)
                .any(|(a, b)| a.to_bits() != b.to_bits())
            || self.counter != o.counter
            || self.remaining != o.remaining
            || self.rows.len() != o.rows.len()
            || self.rows.iter().zip(&o.rows).any(|(a, b)| !row_eq(a, b))
    }

    /// Replaces the settings, validated as `reconfigure` validates them.
    ///
    /// # Errors
    ///
    /// The validation's error; [`Error::HostRateFixed`] for another host rate.
    pub fn set_settings(&mut self, s: Settings) -> Result<(), Error> {
        if s.host_rate_hz != self.settings.host_rate_hz {
            return Err(Error::HostRateFixed);
        }
        s.validate()?;
        self.settings = s;
        Ok(())
    }

    /// Sets the slot's component.
    pub fn set_component(&mut self, c: ComponentId) {
        self.component = c;
    }

    /// Writes store value `index`, validated as `Pipeline::set_component_parameter` validates it.
    ///
    /// # Errors
    ///
    /// [`Error::NoSuchComponent`] (its field carries the store index),
    /// [`Error::InvalidControlValue`].
    pub fn set_parameter(&mut self, index: u32, value: f64) -> Result<(), Error> {
        let (_, p) = components::parameter_at(index as usize)
            .ok_or(Error::NoSuchComponent { component: index })?;
        let v = components::store_value(p, value).ok_or(Error::InvalidControlValue {
            control: p.control_index,
            value,
        })?;
        self.store[index as usize] = v;
        Ok(())
    }

    /// Sets the seed.
    pub fn set_seed(&mut self, seed: u64) {
        self.settings.seed = seed;
    }
    /// Sets the recording counter.
    pub fn set_counter(&mut self, counter: u64) {
        self.counter = counter;
    }
    /// Sets the remaining capacity at the start (checked at [`ReRender::new`]).
    pub fn set_remaining_samples(&mut self, remaining: u64) {
        self.remaining = remaining;
    }
    /// Collects the run (the default) or not (a digest-only session).
    pub fn set_collect_run(&mut self, collect: bool) {
        self.collect_run = collect;
    }

    /// Inserts a change row, kept in position order after the rows at the same position.
    ///
    /// # Errors
    ///
    /// As [`Plan::set_row`].
    pub fn insert_row(&mut self, row: ChangeEntry) -> Result<(), Error> {
        self.check_row(&row, None)?;
        let at = self.rows.partition_point(|r| r.position <= row.position);
        self.rows.insert(at, row);
        Ok(())
    }

    /// Replaces row `i` (re-sorted, stable).
    ///
    /// # Errors
    ///
    /// [`Error::NoSuchControl`] for `i` past the rows; [`Error::ControlNotOnStream`] for a
    /// control that is not a record-side value or a row on another stream; the value's
    /// validation; [`Error::ScheduleFull`] for a 65th row at one position.
    pub fn set_row(&mut self, i: usize, row: ChangeEntry) -> Result<(), Error> {
        if i >= self.rows.len() {
            return Err(Error::NoSuchControl { control: i as u32 });
        }
        self.check_row(&row, Some(i))?;
        self.rows.remove(i);
        let at = self.rows.partition_point(|r| r.position <= row.position);
        self.rows.insert(at, row);
        Ok(())
    }

    /// Removes row `i`.
    ///
    /// # Errors
    ///
    /// [`Error::NoSuchControl`] for `i` past the rows.
    pub fn remove_row(&mut self, i: usize) -> Result<(), Error> {
        if i >= self.rows.len() {
            return Err(Error::NoSuchControl { control: i as u32 });
        }
        self.rows.remove(i);
        Ok(())
    }

    fn check_row(&self, row: &ChangeEntry, replacing: Option<usize>) -> Result<(), Error> {
        let slot = slot_of(row.control).ok_or(Error::ControlNotOnStream {
            control: row.control,
            stream: Stream::Record,
        })?;
        if row.stream != STREAM_RECORD {
            return Err(Error::ControlNotOnStream {
                control: row.control,
                stream: Stream::Record,
            });
        }
        record_value(slot, row.after)?;
        let same = self
            .rows
            .iter()
            .enumerate()
            .filter(|&(j, r)| Some(j) != replacing && r.position == row.position)
            .count() as u32;
        if same >= SCHEDULE {
            return Err(Error::ScheduleFull {
                stream: Stream::Record,
            });
        }
        Ok(())
    }

    /// Test-only (hidden): the re-render mutants (spec 027 R-12).
    #[cfg(feature = "__test-hooks")]
    #[doc(hidden)]
    pub fn __set_mutation(&mut self, m: crate::__test_hooks::RerenderMutation) {
        self.mutation = m;
    }

    #[cfg(feature = "__test-hooks")]
    fn mutation(&self) -> crate::__test_hooks::RerenderMutation {
        self.mutation
    }
    #[cfg(not(feature = "__test-hooks"))]
    fn mutation(&self) -> Mutation {
        Mutation::default()
    }
}

/// The committed behavior where the test hooks are off.
#[cfg(not(feature = "__test-hooks"))]
#[derive(Clone, Copy, Default)]
struct Mutation {
    no_split: bool,
    self_stop_fields_dropped: bool,
    store_skipped: bool,
    i24_scale: bool,
    drop_event: bool,
}

fn reading_bits(r: crate::control_map::ControlReading) -> (u8, u64) {
    use crate::control_map::ControlReading as R;
    match r {
        R::Switch(b) => (0, u64::from(b)),
        R::Integer(n) => (1, n),
        R::Continuous(v) => (2, u64::from(v.to_bits())),
        R::Choice(c) => (3, u64::from(c)),
    }
}

fn row_eq(a: &ChangeEntry, b: &ChangeEntry) -> bool {
    a.position == b.position
        && a.control == b.control
        && a.after.to_bits() == b.after.to_bits()
        && a.tag == b.tag
        && a.stream == b.stream
}

/// What one `feed` call did (spec 027 D2).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct Fed {
    /// The host input samples the engine took (after a self-stop, those it took before it).
    pub taken: u64,
    /// The input reached the record's stop position.
    pub reached_stop: bool,
    /// The memory filled and the recording stopped by itself.
    pub self_stopped: bool,
}

/// Why the session ended as it did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Reason {
    /// The rebuilt item was kept.
    Kept,
    /// Nothing was kept (defensive: the engine keeps a sample of any fed input).
    DiscardedEmpty,
}

/// The re-render's verdict (spec 027 D5). It describes processing only.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Verdict {
    /// An unedited plan gave the original's digest.
    Identical,
    /// An unedited plan gave another digest.
    Differs,
    /// An edited plan (see [`Outcome::matches_original`]).
    Rebuilt,
}

/// The result of [`ReRender::finish`] (spec 027 D5).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct Outcome {
    /// Kept or nothing kept.
    pub reason: Reason,
    /// The rebuilt item's digest (`None` when nothing was kept).
    pub digest: Option<u64>,
    /// The plan's values differ from the record's.
    pub edited: bool,
    /// The digest equals the record's `recording_digest`.
    pub matches_original: bool,
    /// The verdict.
    pub verdict: Verdict,
    /// The record's flags (`record::flags`).
    pub flags: u32,
    /// The remaining capacity the fill reached, in device samples.
    pub remaining_at_start: u64,
    /// The change rows the engine applied. An edited plan's rows past the input it took (after
    /// an earlier self-stop, past the stop's flush, or more than the schedule's 64 pending at
    /// the stop) do not land and are not counted.
    pub rows_landed: u64,
}

/// A streaming re-render session (spec 027 D2).
pub struct ReRender {
    p: Pipeline,
    rows: Vec<ChangeEntry>,
    next: usize,
    stop_position: u64,
    input: u64,
    kind: u8, // 0 none, 1 i24, 2 f32
    self_stopped: bool,
    stop: StopFields,
    ev: Vec<VasEvent>,
    ch: Vec<ChangeEntry>,
    run_events: Vec<VasEvent>,
    run_changes: Vec<ChangeEntry>,
    n_events: usize,
    n_changes: usize,
    rows_landed: u64,
    collect: bool,
    dropped_one: bool,
    expected_digest: u64,
    edited: bool,
    flags: u32,
    remaining_at_start: u64,
    outcome: Option<Outcome>,
    number: u32,
    #[cfg(feature = "__test-hooks")]
    m: crate::__test_hooks::RerenderMutation,
    #[cfg(not(feature = "__test-hooks"))]
    m: Mutation,
}

impl core::fmt::Debug for ReRender {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ReRender")
            .field("input", &self.input)
            .field("stop_position", &self.stop_position)
            .field("outcome", &self.outcome)
            .finish_non_exhaustive()
    }
}

impl ReRender {
    /// Builds the engine, restores the slot and its store, fills the memory and reserves every
    /// buffer (allocates).
    ///
    /// # Errors
    ///
    /// [`Error::ImportedRecord`], [`Error::UnknownComponent`], [`Error::MemoryFull`] (a
    /// remaining capacity of 0), [`Error::DoesNotFit`] (above the capacity),
    /// [`Error::RunTooLarge`], and the engine's own construction errors.
    pub fn new(plan: Plan) -> Result<ReRender, Error> {
        if plan.readout.origin == Origin::Imported {
            return Err(Error::ImportedRecord);
        }
        if plan.readout.flags & record::flags::UNKNOWN_COMPONENT != 0 {
            return Err(Error::UnknownComponent);
        }
        if plan.remaining == 0 {
            return Err(Error::MemoryFull);
        }
        let m = plan.mutation();
        let mut p = Pipeline::new_at_counter(plan.settings, plan.counter)?;
        #[cfg(feature = "__test-hooks")]
        p.__set_rerender_mutation(m);
        if !m.store_skipped {
            for c in components::components() {
                for i in 0..c.parameters.len() as u32 {
                    let at = c.parameter_offset + i;
                    p.set_component_parameter(
                        BeforeLimiter,
                        at,
                        f64::from(plan.store[at as usize]),
                    )?;
                }
            }
        }
        p.set_component(BeforeLimiter, plan.component)?;
        let now = p.memory_status().remaining_samples;
        if plan.remaining > now {
            return Err(Error::DoesNotFit);
        }
        p.fill_to_remaining(plan.remaining)?;
        let remaining_at_start = p.memory_status().remaining_samples;

        // Buffers (plan R-04): the shortest hang over the settings and every hang row.
        let hang_control = crate::controls::controls()
            .iter()
            .position(|c| c.identifier == "vas.hang_ms")
            .map(|i| i as u32);
        let hang = plan
            .rows
            .iter()
            .filter(|r| Some(r.control) == hang_control)
            .map(|r| r.after as f32)
            .fold(plan.settings.vas.hang_ms, f32::min);
        let rate = u64::from(plan.settings.host_rate_hz);
        let latency = u64::from(p.record_latency_samples());
        let flush = latency + 180 * rate.div_ceil(6000) + 2 * latency + 16;
        let stop_position = plan.readout.stop.stop_input_position;
        let bound = |frames: u64| p.recorder.max_events(frames as usize, hang) as u64 + 4;
        let scratch = bound(CHUNK as u64).max(bound(flush)) as usize;
        let run_bound = bound(stop_position + flush);
        let (run_events, run_changes) = if plan.collect_run {
            if run_bound > MAX_RUN_EVENTS {
                return Err(Error::RunTooLarge { events: run_bound });
            }
            (
                vec![VasEvent::default(); run_bound as usize],
                vec![ChangeEntry::default(); plan.rows.len() + MAX_CHANGES_PER_CALL],
            )
        } else {
            (Vec::new(), Vec::new())
        };
        let edited = plan.edited();
        let mut r = ReRender {
            p,
            next: 0,
            stop_position,
            input: 0,
            kind: 0,
            self_stopped: false,
            stop: StopFields::default(),
            ev: vec![VasEvent::default(); scratch],
            ch: vec![ChangeEntry::default(); MAX_CHANGES_PER_CALL],
            run_events,
            run_changes,
            n_events: 0,
            n_changes: 0,
            rows_landed: 0,
            collect: plan.collect_run,
            dropped_one: false,
            expected_digest: plan.readout.recording_digest,
            edited,
            flags: plan.readout.flags,
            remaining_at_start,
            outcome: None,
            number: 0,
            m,
            rows: plan.rows,
        };
        r.top_up()?;
        r.p.start_recording()?;
        Ok(r)
    }

    fn top_up(&mut self) -> Result<(), Error> {
        while self.next < self.rows.len() && self.p.pending_changes(Stream::Record)? < SCHEDULE {
            let c = self.rows[self.next];
            self.p
                .schedule_change(Stream::Record, c.position, c.control, c.after, c.tag)?;
            self.next += 1;
        }
        Ok(())
    }

    /// Feeds 24-bit codes (spec 020's feed point; `input::samples_from_i24`).
    ///
    /// # Errors
    ///
    /// [`Error::InvalidSample`] (nothing taken), [`Error::FeedKindMixed`],
    /// [`Error::ReRenderState`] after `finish`, and the engine's errors.
    pub fn feed(&mut self, codes: &[i32]) -> Result<Fed, Error> {
        self.kind(1)?;
        if codes.iter().any(|&c| !(-(1 << 23)..(1 << 23)).contains(&c)) {
            return Err(Error::InvalidSample);
        }
        let mut buf = [0.0f32; CHUNK];
        let mut fed = Fed::default();
        for block in codes.chunks(CHUNK) {
            let x = &mut buf[..block.len()];
            if self.m.i24_scale {
                for (d, &s) in x.iter_mut().zip(block) {
                    *d = s as f32 / 8_388_607.0;
                }
            } else {
                crate::input::samples_from_i24(block, x);
            }
            let f = self.take(x)?;
            fed.taken += f.taken;
            fed.reached_stop = f.reached_stop;
            fed.self_stopped = f.self_stopped;
            if f.reached_stop || f.self_stopped {
                break;
            }
        }
        Ok(fed)
    }

    /// Feeds engine samples (a host that kept them losslessly; identical only for samples equal
    /// to `samples_from_i24` of the original codes).
    ///
    /// # Errors
    ///
    /// As [`ReRender::feed`] but `InvalidSample`.
    pub fn feed_f32(&mut self, samples: &[f32]) -> Result<Fed, Error> {
        self.kind(2)?;
        self.take(samples)
    }

    fn kind(&mut self, k: u8) -> Result<(), Error> {
        if self.outcome.is_some() {
            return Err(Error::ReRenderState);
        }
        if self.kind != 0 && self.kind != k {
            return Err(Error::FeedKindMixed);
        }
        self.kind = k;
        Ok(())
    }

    /// The split-and-top-up rule (plan R-03).
    fn take(&mut self, mut x: &[f32]) -> Result<Fed, Error> {
        let start = self.input;
        while !x.is_empty() && !self.self_stopped && self.input < self.stop_position {
            let mut n = x.len().min(CHUNK);
            n = n.min((self.stop_position - self.input) as usize);
            if self.next < self.rows.len() && !self.m.no_split {
                let to_row = self.rows[self.next].position.saturating_sub(self.input);
                debug_assert!(to_row > 0, "a row is never late (plan R-03)");
                n = n.min(to_row.max(1) as usize);
            }
            let before = self.input;
            let info = self
                .p
                .record_with_changes(&x[..n], &mut self.ev, &mut self.ch)?;
            self.collect_call(info.events, info.changes)?;
            self.input = info.input_samples_total;
            let used = (self.input - before) as usize;
            x = &x[used.min(x.len())..];
            if info.stopped.is_some() {
                self.self_stopped = true;
                if !self.m.self_stop_fields_dropped {
                    self.stop = StopFields::from(&info);
                }
                break;
            }
            if !self.m.no_split {
                self.top_up()?;
            }
        }
        if self.m.no_split {
            self.top_up()?;
        }
        Ok(Fed {
            taken: self.input - start,
            reached_stop: self.input >= self.stop_position,
            self_stopped: self.self_stopped,
        })
    }

    fn collect_call(&mut self, events: usize, changes: usize) -> Result<(), Error> {
        if events > self.ev.len() || changes > self.ch.len() {
            debug_assert!(false, "a buffer bound is short (plan R-04)");
            return Err(Error::InconsistentRun {
                reason: "a re-render buffer bound is short",
            });
        }
        self.rows_landed += changes as u64;
        if !self.collect {
            return Ok(());
        }
        let mut evs = &self.ev[..events];
        if self.m.drop_event && !self.dropped_one && !evs.is_empty() {
            evs = &evs[1..];
            self.dropped_one = true;
        }
        let (e0, c0) = (self.n_events, self.n_changes);
        if e0 + evs.len() > self.run_events.len() || c0 + changes > self.run_changes.len() {
            debug_assert!(false, "the run bound is short (plan R-04)");
            return Err(Error::InconsistentRun {
                reason: "the re-render's run bound is short",
            });
        }
        self.run_events[e0..e0 + evs.len()].copy_from_slice(evs);
        self.run_changes[c0..c0 + changes].copy_from_slice(&self.ch[..changes]);
        self.n_events += evs.len();
        self.n_changes += changes;
        Ok(())
    }

    /// The input position and the record's stop position, in host samples (progress). After a
    /// self-stop the input stays below the stop position: treat `reached_stop ||
    /// self_stopped` as done.
    pub fn progress(&self) -> (u64, u64) {
        (self.input, self.stop_position)
    }

    /// The engine's meter read-out of its last call, 1 … [`CHUNK`] host samples (a call ends
    /// early at a row it has to schedule; spec 016).
    pub fn meters(&self) -> Meters {
        self.p.meters()
    }

    /// Stops (unless the recording stopped by itself) and reads the rebuilt item. On
    /// [`Error::InputShort`] the session stays feedable.
    ///
    /// # Errors
    ///
    /// [`Error::InputShort`], [`Error::ReRenderState`] after a successful `finish`, and the
    /// engine's errors.
    pub fn finish(&mut self) -> Result<Outcome, Error> {
        if self.outcome.is_some() {
            return Err(Error::ReRenderState);
        }
        let mut kept = true;
        if !self.self_stopped {
            if self.input < self.stop_position {
                return Err(Error::InputShort {
                    needed: self.stop_position - self.input,
                });
            }
            self.top_up()?;
            let out = self
                .p
                .stop_recording_with_changes(&mut self.ev, &mut self.ch)?;
            self.collect_call(out.flush.events, out.changes)?;
            kept = matches!(out.result, StopResult::Kept { .. });
            self.stop = StopFields::from(&out);
        }
        let digest = if kept {
            self.number = self.p.memory_status().count;
            Some(self.p.recording_digest(self.number)?)
        } else {
            None
        };
        let matches_original = digest == Some(self.expected_digest);
        let verdict = if self.edited {
            Verdict::Rebuilt
        } else if matches_original {
            Verdict::Identical
        } else {
            Verdict::Differs
        };
        let o = Outcome {
            reason: if kept {
                Reason::Kept
            } else {
                Reason::DiscardedEmpty
            },
            digest,
            edited: self.edited,
            matches_original,
            verdict,
            flags: self.flags,
            remaining_at_start: self.remaining_at_start,
            rows_landed: self.rows_landed,
        };
        self.outcome = Some(o);
        Ok(o)
    }

    /// The bytes the session holds: the engine's footprint and its reserved buffers (spec 027
    /// US3 AS1).
    pub fn footprint_bytes(&self) -> usize {
        use core::mem::size_of;
        self.p.footprint_bytes()
            + (self.ev.len() + self.run_events.len()) * size_of::<VasEvent>()
            + (self.ch.len() + self.run_changes.len() + self.rows.len()) * size_of::<ChangeEntry>()
    }

    /// The outcome, after a successful `finish`.
    pub fn outcome(&self) -> Option<Outcome> {
        self.outcome
    }

    fn item(&self) -> Result<Option<u32>, Error> {
        match self.outcome {
            None => Err(Error::ReRenderState),
            Some(o) if o.reason == Reason::Kept => Ok(Some(self.number)),
            Some(_) => Ok(None),
        }
    }

    /// The rebuilt item's stored form's length (0 when nothing was kept).
    ///
    /// # Errors
    ///
    /// [`Error::ReRenderState`] before `finish`.
    pub fn export_len(&self) -> Result<usize, Error> {
        match self.item()? {
            Some(n) => self.p.export_len(n),
            None => Ok(0),
        }
    }

    /// Writes the rebuilt item's stored form.
    ///
    /// # Errors
    ///
    /// [`Error::ReRenderState`] before `finish`; [`Error::BufferTooSmall`].
    pub fn export(&self, out: &mut [u8]) -> Result<usize, Error> {
        match self.item()? {
            Some(n) => self.p.export(n, out),
            None => Ok(0),
        }
    }

    fn run(&self) -> Result<RunData<'_>, Error> {
        if !self.collect {
            return Err(Error::RunNotCollected);
        }
        Ok(RunData {
            events: &self.run_events[..self.n_events],
            changes: &self.run_changes[..self.n_changes],
            stop: self.stop,
        })
    }

    /// The rebuilt item's new processing record's length (0 when nothing was kept).
    ///
    /// # Errors
    ///
    /// [`Error::ReRenderState`] before `finish`; [`Error::RunNotCollected`].
    pub fn record_len(&self) -> Result<usize, Error> {
        match self.item()? {
            Some(n) => self.p.processing_record_len(n, &self.run()?),
            None => Ok(0),
        }
    }

    /// Writes the rebuilt item's new processing record (an ordinary engine record).
    ///
    /// # Errors
    ///
    /// As [`ReRender::record_len`]; [`Error::BufferTooSmall`].
    pub fn record_write(&self, out: &mut [u8]) -> Result<usize, Error> {
        match self.item()? {
            Some(n) => self.p.processing_record_write(n, &self.run()?, out),
            None => Ok(0),
        }
    }

    /// The engine, holding the rebuilt item as its last recording (to play it).
    ///
    /// # Errors
    ///
    /// [`Error::ReRenderState`] before `finish`.
    pub fn into_pipeline(self) -> Result<Pipeline, Error> {
        if self.outcome.is_none() {
            return Err(Error::ReRenderState);
        }
        Ok(self.p)
    }
}

/// The one-call convenience: builds the session, feeds every code and finishes.
///
/// # Errors
///
/// As [`ReRender::new`], [`ReRender::feed`] and [`ReRender::finish`].
pub fn re_render(plan: Plan, codes: &[i32]) -> Result<ReRender, Error> {
    let mut r = ReRender::new(plan)?;
    let _ = r.feed(codes)?;
    let _ = r.finish()?;
    Ok(r)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::changes::ChangeEntry;

    fn plan() -> Plan {
        let r = RecordReadout::default();
        let original = Snapshot {
            settings: r.settings,
            component: r.component,
            store: r.store,
            rows: Vec::new(),
            counter: 0,
            remaining: 100,
        };
        Plan {
            settings: r.settings,
            component: r.component,
            store: r.store,
            rows: Vec::new(),
            counter: 0,
            remaining: 100,
            readout: r,
            collect_run: true,
            original,
            #[cfg(feature = "__test-hooks")]
            mutation: Default::default(),
        }
    }

    const KNEE: u32 = 10;

    #[test]
    fn rerender_rows_sorted_stable_and_limited() {
        let mut p = plan();
        let row = |pos, after, tag| ChangeEntry::new(pos, KNEE, 0.0, after, tag, STREAM_RECORD);
        p.insert_row(row(20, -10.0, 1)).unwrap();
        p.insert_row(row(10, -12.0, 2)).unwrap();
        p.insert_row(row(20, -14.0, 3)).unwrap();
        let tags: Vec<u64> = p.rows().iter().map(|r| r.tag).collect();
        assert_eq!(tags, [2, 1, 3], "position order, stable at equal positions");
        for t in 0..62 {
            p.insert_row(row(20, -10.0, 10 + t)).unwrap();
        }
        assert!(matches!(
            p.insert_row(row(20, -10.0, 99)),
            Err(Error::ScheduleFull { .. })
        ));
        // Moving a row onto the full position is refused too; replacing a row there is not.
        assert!(matches!(
            p.set_row(0, row(20, -10.0, 2)),
            Err(Error::ScheduleFull { .. })
        ));
        p.set_row(1, row(20, -11.0, 1)).unwrap();
        assert!(
            p.insert_row(row(5, 99.0, 1)).is_err(),
            "out of the knee's range"
        );
        assert!(matches!(
            p.insert_row(ChangeEntry::new(5, 2, 0.0, 1.0, 1, STREAM_RECORD)),
            Err(Error::ControlNotOnStream { .. })
        ));
        assert!(matches!(
            p.remove_row(999),
            Err(Error::NoSuchControl { .. })
        ));
    }

    #[test]
    fn rerender_edited_by_bits() {
        let mut p = plan();
        assert!(!p.edited());
        let s = *p.settings();
        p.set_settings(s).unwrap();
        assert!(!p.edited(), "a same-value write");
        let mut s = *p.settings();
        s.mic.hiss_offset_db = -0.0;
        p.set_settings(s).unwrap();
        assert!(p.edited(), "-0.0 by bits");
        let mut q = plan();
        q.set_seed(q.seed() ^ 1);
        assert!(q.edited());
        let mut q = plan();
        q.set_parameter(1, 500.0).unwrap();
        assert!(q.edited());
        assert!(matches!(
            q.set_parameter(99, 1.0),
            Err(Error::NoSuchComponent { .. })
        ));
        let mut q = plan();
        let mut s = *q.settings();
        s.host_rate_hz = 48_000;
        assert_eq!(q.set_settings(s), Err(Error::HostRateFixed));
    }
}
