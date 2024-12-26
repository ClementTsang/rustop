//! In charge of cleaning, processing, and managing data.

use std::{
    collections::BTreeMap,
    time::{Duration, Instant},
    vec::Vec,
};

use hashbrown::HashMap;

#[cfg(feature = "battery")]
use crate::data_collection::batteries;
use crate::{
    data_collection::{
        cpu, disks, memory, network,
        processes::{Pid, ProcessHarvest},
        temperature, Data,
    },
    dec_bytes_per_second_string,
};

/// A chunk of data, corresponding to the indices of time slice.
#[derive(Debug)]
pub struct DataChunk {
    /// The start offset of this chunk, should correspond to the time_offsets. If that updates,
    /// this MUST also update.
    start_offset: usize,

    /// The end offset of this chunk, should correspond to the time_offsets. If that updates,
    /// this MUST also update.
    end_offset: usize,

    /// The actual value data!
    data: Vec<f64>,
}

impl DataChunk {
    /// Create a new [`DataChunk`] starting from `offset`.
    pub fn new(initial_value: f64, start_offset: usize) -> Self {
        Self {
            start_offset,
            end_offset: start_offset + 1,
            data: vec![initial_value],
        }
    }

    /// Try and prune the chunk.
    pub fn try_prune(&mut self, prune_end_index: usize) -> bool {
        if prune_end_index > self.end_offset {
            self.data.clear();
            self.start_offset = 0;
            self.end_offset = 0;

            true
        } else if prune_end_index > self.start_offset {
            // We know the prune index must be between the start and end, so we're safe
            // to blindly do subtaction here, assuming our other invariants held.

            let drain_end = prune_end_index - self.start_offset;

            self.data.drain(..drain_end);

            self.start_offset = 0;
            self.end_offset -= prune_end_index;

            true
        } else {
            false
        }
    }

    /// Update the offsets of this chunk by `usize`.
    pub fn update_indices(&mut self, offset: usize) {
        self.start_offset -= offset;
        self.end_offset -= offset;
    }
}

/// Represents timeseries _value_ data in a chunked fashion.
#[derive(Debug, Default)]
pub struct ValueChunk {
    /// The currently-updated chunk.
    current: Option<DataChunk>,

    /// Previous chunks, this should be added to if a data gap is found.
    previous_chunks: Vec<DataChunk>,
}

impl ValueChunk {
    /// Add a value to this chunk.
    pub fn add(&mut self, value: f64, index: usize) {
        match self.current.as_mut() {
            Some(current) => {
                current.data.push(value);
                current.end_offset = index + 1;
            }
            None => {
                self.current = Some(DataChunk::new(value, index));
            }
        }
    }

    /// End the current chunk.
    pub fn end_chunk(&mut self) {
        if let Some(current) = self.current.take() {
            self.previous_chunks.push(current);
        }
    }

    /// Prune all chunks up to (and not including) the current end index, and update all internal indicies to match this.
    pub fn prune(&mut self, remove_up_to: usize) {
        // Try to prune the current; if we _can_ prune the current, then it likely means all the
        // previous chunks should also be pruned.

        let pruned_current = if let Some(current) = self.current.as_mut() {
            current.try_prune(remove_up_to)
        } else {
            false
        };

        if pruned_current {
            // If we could prune the current chunk, then it means all other chunks are outdated. Remove them.
            if !self.previous_chunks.is_empty() {
                self.previous_chunks.clear();
                self.previous_chunks.shrink_to_fit();
            }
        } else {
            // Otherwise, try and prune the previous chunks + adjust the remaining chunks' offsets.

            for (index, previous_chunk) in self.previous_chunks.iter_mut().enumerate().rev() {
                if previous_chunk.try_prune(remove_up_to) {
                    let end_index = if previous_chunk.end_offset == 0 {
                        index + 1
                    } else {
                        index
                    };

                    self.previous_chunks.drain(0..end_index);

                    if let Some(current) = &mut self.current {
                        current.update_indices(remove_up_to);
                    }

                    for previous_chunk in self.previous_chunks.iter_mut().skip(1) {
                        previous_chunk.update_indices(remove_up_to);
                    }

                    return;
                }
            }
        }
    }

    /// Check if a [`DataChunk`] has no data in it.
    pub fn is_empty(&self) -> bool {
        if let Some(current) = &self.current {
            if !current.data.is_empty() {
                return false;
            }
        }

        // If any of the previous chunks are not empty, return false.
        // If there are no previous chunks, return true.
        !self.previous_chunks.iter().any(|c| !c.data.is_empty())
    }
}

#[derive(Debug, Clone, Copy)]
struct DefaultInstant(Instant);

impl Default for DefaultInstant {
    fn default() -> Self {
        Self(Instant::now())
    }
}

/// Represents timeseries data in a chunked, deduped manner.
///
/// Properties:
/// - Time in this manner is represented in a reverse-offset fashion from the current time.
/// - All data is stored in SoA fashion.
/// - Values are stored in a chunked format, which facilitates gaps in data collection if needed.
/// - Additional metadata is stored to make data pruning over time easy.
#[derive(Debug, Default)]
pub struct TimeSeriesData {
    /// The last-updated timestamp. The last time offset is based on this value.
    /// When updating this value ensure you also update time_offsets with the
    /// new offset from the new time to the original value.
    current_time: DefaultInstant,

    /// All time offsets relative to the previous value, first element is the oldest value,
    /// and is relvative to `current_time`.
    ///
    /// For example:
    /// [1, 5, 3], with current_time of 9 and starting initially from 0,
    /// would represent values of [0, 1, 6, 9], 9 being the last-read value.
    ///
    /// We store this as u32 to save memory; in theory we can store this as
    /// an even smaller, compressible data format.
    time_offsets: Vec<u32>,

    /// Time offset ranges to help faciliate pruning. Must be in
    /// sorted order. Offset ranges are [start, end) (that is, exclusive).
    ///
    /// Storing double usize might be wasteful but eh.
    offset_ranges: Vec<(Instant, usize, usize)>,

    /// Network RX data chunks.
    rx: ValueChunk,

    /// Network TX data chunks.
    tx: ValueChunk,

    /// CPU data chunks.
    cpu: Vec<ValueChunk>,

    /// Memory data chunks.
    mem: ValueChunk,

    /// Swap data chunks.
    swap: ValueChunk,

    #[cfg(not(target_os = "windows"))]
    /// Cache data chunks.
    cache_mem: ValueChunk,

    #[cfg(feature = "zfs")]
    /// Arc data chunks.
    arc_mem: ValueChunk,

    #[cfg(feature = "gpu")]
    /// GPU memory data chunks.
    gpu_mem: Vec<ValueChunk>,
}

impl TimeSeriesData {
    /// Add a new data point.
    pub fn add(&mut self, data: Data) {
        let time = data
            .collection_time
            .duration_since(self.current_time.0)
            .as_millis() as u32;
        self.current_time.0 = data.collection_time;
        self.time_offsets.push(time);

        let index = self.time_offsets.len() - 1;

        if let Some(network) = data.network {
            self.rx.add(network.rx as f64, index);
            self.tx.add(network.tx as f64, index);
        }

        if let Some(cpu) = data.cpu {
            for (itx, c) in cpu.into_iter().enumerate() {
                todo!()
            }
        }

        if let Some(memory) = data.memory {
            if let Some(val) = memory.checked_percent() {
                self.mem.add(val, index);
            } else {
                self.mem.end_chunk();
            }
        }

        if let Some(swap) = data.swap {
            if let Some(val) = swap.checked_percent() {
                self.swap.add(val, index);
            } else {
                self.swap.end_chunk();
            }
        }

        #[cfg(not(target_os = "windows"))]
        if let Some(cache) = data.cache {
            if let Some(val) = cache.checked_percent() {
                self.cache_mem.add(val, index);
            } else {
                self.cache_mem.end_chunk();
            }
        }

        #[cfg(feature = "zfs")]
        if let Some(arc) = data.arc {
            if let Some(val) = arc.checked_percent() {
                self.arc_mem.add(val, index);
            } else {
                self.arc_mem.end_chunk();
            }
        }

        #[cfg(feature = "gpu")]
        if let Some(gpu) = data.gpu {
            for g in gpu {
                todo!()
            }
        }
    }

    /// Prune any data older than the given duration.
    pub fn prune(&mut self, max_age: Duration) {
        let remove_index = match self.offset_ranges.binary_search_by(|(instant, _, _)| {
            self.current_time
                .0
                .duration_since(*instant)
                .cmp(&max_age)
                .reverse()
        }) {
            Ok(index) => index,
            Err(index) => index,
        };

        if let Some((_offset, start, end)) = self.offset_ranges.drain(0..remove_index).last() {
            // Note that end here is _exclusive_.
            self.time_offsets.drain(0..end);

            self.rx.prune(end);
            self.tx.prune(end);

            // TODO: Maybe make a wrapper around a Vec<DataChunk>?
            {
                let mut to_delete = vec![];

                for (itx, cpu) in self.cpu.iter_mut().enumerate() {
                    cpu.prune(end);

                    // We don't want to retain things if there is no data at all.
                    if cpu.is_empty() {
                        to_delete.push(itx);
                    }
                }

                for itx in to_delete.into_iter().rev() {
                    self.cpu.remove(itx);
                }
            }

            self.mem.prune(end);
            self.swap.prune(end);

            #[cfg(not(target_os = "windows"))]
            self.cache_mem.prune(end);

            #[cfg(feature = "zfs")]
            self.arc_mem.prune(end);

            #[cfg(feature = "gpu")]
            {
                let mut to_delete = vec![];

                for (itx, gpu) in self.gpu_mem.iter_mut().enumerate() {
                    gpu.prune(end);

                    // We don't want to retain things if there is no data at all.
                    if gpu.is_empty() {
                        to_delete.push(itx);
                    }
                }

                for itx in to_delete.into_iter().rev() {
                    self.gpu_mem.remove(itx);
                }
            }
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct TimedData {
    pub rx_data: f64,
    pub tx_data: f64,
    pub cpu_data: Vec<f64>,
    pub mem_data: Option<f64>,
    #[cfg(not(target_os = "windows"))]
    pub cache_data: Option<f64>,
    pub swap_data: Option<f64>,
    #[cfg(feature = "zfs")]
    pub arc_data: Option<f64>,
    #[cfg(feature = "gpu")]
    pub gpu_data: Vec<Option<f64>>,
}

#[derive(Clone, Debug, Default)]
pub struct ProcessData {
    /// A PID to process data map.
    pub process_harvest: BTreeMap<Pid, ProcessHarvest>,

    /// A mapping between a process PID to any children process PIDs.
    pub process_parent_mapping: HashMap<Pid, Vec<Pid>>,

    /// PIDs corresponding to processes that have no parents.
    pub orphan_pids: Vec<Pid>,
}

impl ProcessData {
    fn ingest(&mut self, list_of_processes: Vec<ProcessHarvest>) {
        self.process_parent_mapping.clear();

        // Reverse as otherwise the pid mappings are in the wrong order.
        list_of_processes.iter().rev().for_each(|process_harvest| {
            if let Some(parent_pid) = process_harvest.parent_pid {
                if let Some(entry) = self.process_parent_mapping.get_mut(&parent_pid) {
                    entry.push(process_harvest.pid);
                } else {
                    self.process_parent_mapping
                        .insert(parent_pid, vec![process_harvest.pid]);
                }
            }
        });

        self.process_parent_mapping.shrink_to_fit();

        let process_pid_map = list_of_processes
            .into_iter()
            .map(|process| (process.pid, process))
            .collect();
        self.process_harvest = process_pid_map;

        // We collect all processes that either:
        // - Do not have a parent PID (that is, they are orphan processes)
        // - Have a parent PID but we don't have the parent (we promote them as orphans)
        self.orphan_pids = self
            .process_harvest
            .iter()
            .filter_map(|(pid, process_harvest)| match process_harvest.parent_pid {
                Some(parent_pid) if self.process_harvest.contains_key(&parent_pid) => None,
                _ => Some(*pid),
            })
            .collect();
    }
}

/// AppCollection represents the pooled data stored within the main app
/// thread.  Basically stores a (occasionally cleaned) record of the data
/// collected, and what is needed to convert into a displayable form.
///
/// If the app is *frozen* - that is, we do not want to *display* any changing
/// data, keep updating this. As of 2021-09-08, we just clone the current
/// collection when it freezes to have a snapshot floating around.
///
/// Note that with this method, the *app* thread is responsible for cleaning -
/// not the data collector.
#[derive(Debug, Clone)]
pub struct DataCollection {
    pub current_instant: Instant,
    pub timed_data_vec: Vec<(Instant, TimedData)>,
    pub network_harvest: network::NetworkHarvest,
    pub memory_harvest: memory::MemHarvest,
    #[cfg(not(target_os = "windows"))]
    pub cache_harvest: memory::MemHarvest,
    pub swap_harvest: memory::MemHarvest,
    pub cpu_harvest: cpu::CpuHarvest,
    pub load_avg_harvest: cpu::LoadAvgHarvest,
    pub process_data: ProcessData,
    pub disk_harvest: Vec<disks::DiskHarvest>,
    pub io_harvest: disks::IoHarvest,
    pub io_labels_and_prev: Vec<((u64, u64), (u64, u64))>,
    pub io_labels: Vec<(String, String)>,
    pub temp_harvest: Vec<temperature::TempHarvest>,
    #[cfg(feature = "battery")]
    pub battery_harvest: Vec<batteries::BatteryData>,
    #[cfg(feature = "zfs")]
    pub arc_harvest: memory::MemHarvest,
    #[cfg(feature = "gpu")]
    pub gpu_harvest: Vec<(String, memory::MemHarvest)>,
}

impl Default for DataCollection {
    fn default() -> Self {
        DataCollection {
            current_instant: Instant::now(),
            timed_data_vec: Vec::default(),
            network_harvest: network::NetworkHarvest::default(),
            memory_harvest: memory::MemHarvest::default(),
            #[cfg(not(target_os = "windows"))]
            cache_harvest: memory::MemHarvest::default(),
            swap_harvest: memory::MemHarvest::default(),
            cpu_harvest: cpu::CpuHarvest::default(),
            load_avg_harvest: cpu::LoadAvgHarvest::default(),
            process_data: Default::default(),
            disk_harvest: Vec::default(),
            io_harvest: disks::IoHarvest::default(),
            io_labels_and_prev: Vec::default(),
            io_labels: Vec::default(),
            temp_harvest: Vec::default(),
            #[cfg(feature = "battery")]
            battery_harvest: Vec::default(),
            #[cfg(feature = "zfs")]
            arc_harvest: memory::MemHarvest::default(),
            #[cfg(feature = "gpu")]
            gpu_harvest: Vec::default(),
        }
    }
}

impl DataCollection {
    pub fn reset(&mut self) {
        self.timed_data_vec = Vec::default();
        self.network_harvest = network::NetworkHarvest::default();
        self.memory_harvest = memory::MemHarvest::default();
        self.swap_harvest = memory::MemHarvest::default();
        self.cpu_harvest = cpu::CpuHarvest::default();
        self.process_data = Default::default();
        self.disk_harvest = Vec::default();
        self.io_harvest = disks::IoHarvest::default();
        self.io_labels_and_prev = Vec::default();
        self.temp_harvest = Vec::default();
        #[cfg(feature = "battery")]
        {
            self.battery_harvest = Vec::default();
        }
        #[cfg(feature = "zfs")]
        {
            self.arc_harvest = memory::MemHarvest::default();
        }
        #[cfg(feature = "gpu")]
        {
            self.gpu_harvest = Vec::default();
        }
    }

    pub fn clean_data(&mut self, max_time_millis: u64) {
        let current_time = Instant::now();

        let remove_index = match self
            .timed_data_vec
            .binary_search_by(|(instant, _timed_data)| {
                current_time
                    .duration_since(*instant)
                    .as_millis()
                    .cmp(&(max_time_millis.into()))
                    .reverse()
            }) {
            Ok(index) => index,
            Err(index) => index,
        };

        self.timed_data_vec.drain(0..remove_index);
        self.timed_data_vec.shrink_to_fit();
    }

    #[allow(
        clippy::boxed_local,
        reason = "Clippy allow to avoid warning on certain platforms (e.g. 32-bit)."
    )]
    pub fn eat_data(&mut self, harvested_data: Box<Data>) {
        let harvested_time = harvested_data.collection_time;
        let mut new_entry = TimedData::default();

        // Network
        if let Some(network) = harvested_data.network {
            self.eat_network(network, &mut new_entry);
        }

        // Memory, Swap
        if let (Some(memory), Some(swap)) = (harvested_data.memory, harvested_data.swap) {
            self.eat_memory_and_swap(memory, swap, &mut new_entry);
        }

        // Cache memory
        #[cfg(not(target_os = "windows"))]
        if let Some(cache) = harvested_data.cache {
            self.eat_cache(cache, &mut new_entry);
        }

        #[cfg(feature = "zfs")]
        if let Some(arc) = harvested_data.arc {
            self.eat_arc(arc, &mut new_entry);
        }

        #[cfg(feature = "gpu")]
        if let Some(gpu) = harvested_data.gpu {
            self.eat_gpu(gpu, &mut new_entry);
        }

        // CPU
        if let Some(cpu) = harvested_data.cpu {
            self.eat_cpu(cpu, &mut new_entry);
        }

        // Load average
        if let Some(load_avg) = harvested_data.load_avg {
            self.eat_load_avg(load_avg);
        }

        // Temp
        if let Some(temperature_sensors) = harvested_data.temperature_sensors {
            self.eat_temp(temperature_sensors);
        }

        // Disks
        if let Some(disks) = harvested_data.disks {
            if let Some(io) = harvested_data.io {
                self.eat_disks(disks, io, harvested_time);
            }
        }

        // Processes
        if let Some(list_of_processes) = harvested_data.list_of_processes {
            self.eat_proc(list_of_processes);
        }

        #[cfg(feature = "battery")]
        {
            // Battery
            if let Some(list_of_batteries) = harvested_data.list_of_batteries {
                self.eat_battery(list_of_batteries);
            }
        }

        // And we're done eating.  Update time and push the new entry!
        self.current_instant = harvested_time;
        self.timed_data_vec.push((harvested_time, new_entry));
    }

    fn eat_memory_and_swap(
        &mut self, memory: memory::MemHarvest, swap: memory::MemHarvest, new_entry: &mut TimedData,
    ) {
        new_entry.mem_data = memory.checked_percent();
        new_entry.swap_data = swap.checked_percent();

        // In addition copy over latest data for easy reference
        self.memory_harvest = memory;
        self.swap_harvest = swap;
    }

    #[cfg(not(target_os = "windows"))]
    fn eat_cache(&mut self, cache: memory::MemHarvest, new_entry: &mut TimedData) {
        new_entry.cache_data = cache.checked_percent();
        self.cache_harvest = cache;
    }

    fn eat_network(&mut self, network: network::NetworkHarvest, new_entry: &mut TimedData) {
        // RX
        if network.rx > 0 {
            new_entry.rx_data = network.rx as f64;
        }

        // TX
        if network.tx > 0 {
            new_entry.tx_data = network.tx as f64;
        }

        // In addition copy over latest data for easy reference
        self.network_harvest = network;
    }

    fn eat_cpu(&mut self, cpu: Vec<cpu::CpuData>, new_entry: &mut TimedData) {
        // Note this only pre-calculates the data points - the names will be
        // within the local copy of cpu_harvest.  Since it's all sequential
        // it probably doesn't matter anyways.
        cpu.iter()
            .for_each(|cpu| new_entry.cpu_data.push(cpu.cpu_usage));

        self.cpu_harvest = cpu;
    }

    fn eat_load_avg(&mut self, load_avg: cpu::LoadAvgHarvest) {
        self.load_avg_harvest = load_avg;
    }

    fn eat_temp(&mut self, temperature_sensors: Vec<temperature::TempHarvest>) {
        self.temp_harvest = temperature_sensors;
    }

    fn eat_disks(
        &mut self, disks: Vec<disks::DiskHarvest>, io: disks::IoHarvest, harvested_time: Instant,
    ) {
        let time_since_last_harvest = harvested_time
            .duration_since(self.current_instant)
            .as_secs_f64();

        for (itx, device) in disks.iter().enumerate() {
            let checked_name = {
                #[cfg(target_os = "windows")]
                {
                    match &device.volume_name {
                        Some(volume_name) => Some(volume_name.as_str()),
                        None => device.name.split('/').last(),
                    }
                }
                #[cfg(not(target_os = "windows"))]
                {
                    #[cfg(feature = "zfs")]
                    {
                        if !device.name.starts_with('/') {
                            Some(device.name.as_str()) // use the whole zfs
                                                       // dataset name
                        } else {
                            device.name.split('/').last()
                        }
                    }
                    #[cfg(not(feature = "zfs"))]
                    {
                        device.name.split('/').last()
                    }
                }
            };

            if let Some(checked_name) = checked_name {
                let io_device = {
                    #[cfg(target_os = "macos")]
                    {
                        use std::sync::OnceLock;

                        use regex::Regex;

                        // Must trim one level further for macOS!
                        static DISK_REGEX: OnceLock<Regex> = OnceLock::new();

                        #[expect(
                            clippy::regex_creation_in_loops,
                            reason = "this is fine since it's done via a static OnceLock. In the future though, separate it out."
                        )]
                        if let Some(new_name) = DISK_REGEX
                            .get_or_init(|| Regex::new(r"disk\d+").unwrap())
                            .find(checked_name)
                        {
                            io.get(new_name.as_str())
                        } else {
                            None
                        }
                    }
                    #[cfg(not(target_os = "macos"))]
                    {
                        io.get(checked_name)
                    }
                };

                if let Some(io_device) = io_device {
                    let (io_r_pt, io_w_pt) = if let Some(io) = io_device {
                        (io.read_bytes, io.write_bytes)
                    } else {
                        (0, 0)
                    };

                    if self.io_labels.len() <= itx {
                        self.io_labels.push((String::default(), String::default()));
                    }

                    if self.io_labels_and_prev.len() <= itx {
                        self.io_labels_and_prev.push(((0, 0), (io_r_pt, io_w_pt)));
                    }

                    if let Some((io_curr, io_prev)) = self.io_labels_and_prev.get_mut(itx) {
                        let r_rate = ((io_r_pt.saturating_sub(io_prev.0)) as f64
                            / time_since_last_harvest)
                            .round() as u64;
                        let w_rate = ((io_w_pt.saturating_sub(io_prev.1)) as f64
                            / time_since_last_harvest)
                            .round() as u64;

                        *io_curr = (r_rate, w_rate);
                        *io_prev = (io_r_pt, io_w_pt);

                        // TODO: idk why I'm generating this here tbh
                        if let Some(io_labels) = self.io_labels.get_mut(itx) {
                            *io_labels = (
                                dec_bytes_per_second_string(r_rate),
                                dec_bytes_per_second_string(w_rate),
                            );
                        }
                    }
                } else {
                    if self.io_labels.len() <= itx {
                        self.io_labels.push((String::default(), String::default()));
                    }

                    if let Some(io_labels) = self.io_labels.get_mut(itx) {
                        *io_labels = ("N/A".to_string(), "N/A".to_string());
                    }
                }
            }
        }

        self.disk_harvest = disks;
        self.io_harvest = io;
    }

    fn eat_proc(&mut self, list_of_processes: Vec<ProcessHarvest>) {
        self.process_data.ingest(list_of_processes);
    }

    #[cfg(feature = "battery")]
    fn eat_battery(&mut self, list_of_batteries: Vec<batteries::BatteryData>) {
        self.battery_harvest = list_of_batteries;
    }

    #[cfg(feature = "zfs")]
    fn eat_arc(&mut self, arc: memory::MemHarvest, new_entry: &mut TimedData) {
        new_entry.arc_data = arc.checked_percent();
        self.arc_harvest = arc;
    }

    #[cfg(feature = "gpu")]
    fn eat_gpu(&mut self, gpu: Vec<(String, memory::MemHarvest)>, new_entry: &mut TimedData) {
        // Note this only pre-calculates the data points - the names will be
        // within the local copy of gpu_harvest. Since it's all sequential
        // it probably doesn't matter anyways.
        gpu.iter().for_each(|data| {
            new_entry.gpu_data.push(data.1.checked_percent());
        });
        self.gpu_harvest = gpu;
    }
}

#[cfg(test)]
mod test {
    use super::*;

    /// Basic sanity test for current chunk adding/pruning behaviour.

    #[test]
    fn prune_current_chunk() {
        let mut vc = ValueChunk::default();
        let times = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0];

        let mut index = 1;
        for time in &times[index..] {
            vc.add(*time * 2.0, index);
            index += 1
        }

        assert_eq!(
            (&vc).current.as_ref().unwrap().data,
            &[4.0, 6.0, 8.0, 10.0, 12.0]
        );
        assert_eq!((&vc).current.as_ref().unwrap().start_offset, 1);
        assert_eq!((&vc).current.as_ref().unwrap().end_offset, 6);

        // Test removing partially.
        vc.prune(3);
        assert_eq!((&vc).current.as_ref().unwrap().data, &[8.0, 10.0, 12.0]);
        assert_eq!((&vc).current.as_ref().unwrap().start_offset, 0);
        assert_eq!((&vc).current.as_ref().unwrap().end_offset, 3);

        // Test fully clearing house.
        vc.prune(3);
        assert_eq!((&vc).current.as_ref().unwrap().data, &[]);
        assert_eq!((&vc).current.as_ref().unwrap().start_offset, 0);
        assert_eq!((&vc).current.as_ref().unwrap().end_offset, 0);

        // Test re-adding values and clearing again.
        let second_input = [1.0, 2.0, 3.0, 4.0];
        for (index, val) in second_input.into_iter().enumerate() {
            vc.add(val, index);
        }

        assert_eq!((&vc).current.as_ref().unwrap().data, &second_input);
        assert_eq!((&vc).current.as_ref().unwrap().start_offset, 0);
        assert_eq!((&vc).current.as_ref().unwrap().end_offset, 4);

        vc.prune(3);
        assert_eq!((&vc).current.as_ref().unwrap().data, &[4.0]);
        assert_eq!((&vc).current.as_ref().unwrap().start_offset, 0);
        assert_eq!((&vc).current.as_ref().unwrap().end_offset, 1);

        vc.prune(0);
        assert_eq!((&vc).current.as_ref().unwrap().data, &[4.0]);
        assert_eq!((&vc).current.as_ref().unwrap().start_offset, 0);
        assert_eq!((&vc).current.as_ref().unwrap().end_offset, 1);

        vc.prune(1);
        assert_eq!((&vc).current.as_ref().unwrap().data, &[]);
        assert_eq!((&vc).current.as_ref().unwrap().start_offset, 0);
        assert_eq!((&vc).current.as_ref().unwrap().end_offset, 0);
    }

    /// Test pruning multiple chunks.
    #[test]
    fn prune_multi() {
        // Let's simulate the following:
        //
        // |_________________|_________________|____________|
        // 0    chunk 1      5     no data    10  chunk 2   20

        let mut vc = ValueChunk::default();

        for i in 0..5 {
            vc.add((i * 10) as f64, i);
        }

        vc.end_chunk();

        for i in 10..20 {
            vc.add((i * 100) as f64, i);
        }

        assert!(vc.current.is_some());
        assert_eq!(vc.previous_chunks.len(), 1);

        assert_eq!(vc.current.as_ref().unwrap().data.len(), 10);
        assert_eq!(vc.current.as_ref().unwrap().start_offset, 10);
        assert_eq!(vc.current.as_ref().unwrap().end_offset, 20);

        assert_eq!(vc.previous_chunks.get(0).as_ref().unwrap().data.len(), 5);
        assert_eq!(vc.previous_chunks.get(0).as_ref().unwrap().start_offset, 0);
        assert_eq!(vc.previous_chunks.get(0).as_ref().unwrap().end_offset, 5);

        // Try partial pruning previous, make sure it affects current indices too.
        vc.prune(3);

        assert!(vc.current.is_some());
        assert_eq!(vc.previous_chunks.len(), 1);

        assert_eq!(vc.current.as_ref().unwrap().data.len(), 10);
        assert_eq!(vc.current.as_ref().unwrap().start_offset, 7);
        assert_eq!(vc.current.as_ref().unwrap().end_offset, 17);

        assert_eq!(vc.previous_chunks.get(0).as_ref().unwrap().data.len(), 2);
        assert_eq!(vc.previous_chunks.get(0).as_ref().unwrap().start_offset, 0);
        assert_eq!(vc.previous_chunks.get(0).as_ref().unwrap().end_offset, 2);

        // Try full pruning previous.
        vc.prune(2);

        assert!(vc.current.is_some());
        assert!(vc.previous_chunks.is_empty());

        assert_eq!(vc.current.as_ref().unwrap().data.len(), 10);
        assert_eq!(vc.current.as_ref().unwrap().start_offset, 5);
        assert_eq!(vc.current.as_ref().unwrap().end_offset, 15);

        // End chunk, then add a new one. Then end chunk and add a new one. Then end chunk and add a new one.
        vc.end_chunk();
        for i in 15..30 {
            vc.add((i * 1000) as f64, i);
        }

        vc.end_chunk();
        for i in 35..50 {
            vc.add((i * 10000) as f64, i);
        }

        vc.end_chunk();
        for i in 58..60 {
            vc.add((i * 100000) as f64, i);
        }

        assert!(vc.current.is_some());
        assert_eq!(vc.previous_chunks.len(), 3);

        // Ensure current chunk is downgraded to previous_chunks.
        assert_eq!(vc.previous_chunks[0].data.len(), 10);

        // Try pruning the middle chunk, ensure older chunks are cleared and newer chunks are updated.
        vc.prune(25);

        assert!(vc.current.is_some());
        assert_eq!(vc.previous_chunks.len(), 2);

        assert_eq!(vc.previous_chunks.get(0).as_ref().unwrap().data.len(), 5);
        assert_eq!(vc.previous_chunks.get(0).as_ref().unwrap().start_offset, 0);
        assert_eq!(vc.previous_chunks.get(0).as_ref().unwrap().end_offset, 5);

        // Gap of 5, so 5 + 5 = 10
        assert_eq!(vc.previous_chunks.get(1).as_ref().unwrap().data.len(), 15);
        assert_eq!(vc.previous_chunks.get(1).as_ref().unwrap().start_offset, 10);
        assert_eq!(vc.previous_chunks.get(1).as_ref().unwrap().end_offset, 25);

        // Gap of 8, so 25 + 8 = 33
        assert_eq!(vc.current.as_ref().unwrap().data.len(), 2);
        assert_eq!(vc.current.as_ref().unwrap().start_offset, 33);
        assert_eq!(vc.current.as_ref().unwrap().end_offset, 35);

        // Try pruning current. Ensure previous chunks are cleared.
        vc.prune(34);

        assert!(vc.current.is_some());
        assert!(vc.previous_chunks.is_empty());

        assert_eq!(vc.current.as_ref().unwrap().data.len(), 1);
        assert_eq!(vc.current.as_ref().unwrap().start_offset, 0);
        assert_eq!(vc.current.as_ref().unwrap().end_offset, 1);

        vc.prune(1);

        assert!(vc.current.as_ref().unwrap().data.is_empty());
        assert_eq!(vc.current.as_ref().unwrap().start_offset, 0);
        assert_eq!(vc.current.as_ref().unwrap().end_offset, 0);
    }
}
