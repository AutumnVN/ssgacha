use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use chrono::{DateTime, SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::metadata::{
    Banner, BannerOutput, Catalog, FlatHistoryEntry, HistoryEntry, ItemInfo, OutputError, Pity,
    RegionOutput, Spark, Stats, StorageOutput, Totals,
};
use crate::proto::GachaInfo;
use crate::regions::SPARK_THRESHOLD;

#[derive(Clone, Debug)]
pub struct Pull {
    pub banner_id: u32,
    pub storage_id: u32,
    pub time: i64,
    pub ids: Vec<u32>,
}

pub struct FetchedRegion {
    pub uid: String,
    pub catalog: Catalog,
    pub pulls: Vec<Pull>,
    pub gacha_info: Vec<GachaInfo>,
    pub errors: Vec<OutputError>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ExistingRegion {
    #[serde(default)]
    pub uid: Value,
    #[serde(default)]
    pub history: Vec<ExistingPull>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExistingPull {
    pub banner_id: u32,
    pub storage_id: u32,
    pub time: i64,
    #[serde(default)]
    pub ids: Vec<u32>,
}

fn value_string(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Number(number) => number.to_string(),
        Value::Bool(boolean) => boolean.to_string(),
        Value::Null => "null".to_string(),
        other => other.to_string(),
    }
}

fn pull_key(pull: &Pull) -> String {
    format!(
        "{}|{}|{}|{}",
        pull.banner_id,
        pull.storage_id,
        pull.time,
        pull.ids
            .iter()
            .map(u32::to_string)
            .collect::<Vec<_>>()
            .join(",")
    )
}

pub fn merge_pulls(existing: &[Pull], fresh: &[Pull]) -> Vec<Pull> {
    if existing.is_empty() {
        return fresh.to_vec();
    }

    let mut remaining: HashMap<String, usize> = HashMap::new();
    for pull in existing {
        *remaining.entry(pull_key(pull)).or_default() += 1;
    }

    let mut merged = Vec::with_capacity(fresh.len() + existing.len());
    for pull in fresh {
        merged.push(pull.clone());
        if let Some(count) = remaining.get_mut(&pull_key(pull)) {
            if *count > 0 {
                *count -= 1;
            }
        }
    }
    for pull in existing {
        let key = pull_key(pull);
        if let Some(count) = remaining.get_mut(&key) {
            if *count > 0 {
                merged.push(pull.clone());
                *count -= 1;
            }
        }
    }
    merged
}

fn iso_from_unix(seconds: i64) -> String {
    DateTime::<Utc>::from_timestamp(seconds, 0)
        .unwrap_or(DateTime::<Utc>::UNIX_EPOCH)
        .to_rfc3339_opts(SecondsFormat::Millis, true)
}

fn rate_up_ids(items: &[Value]) -> HashSet<u32> {
    items
        .iter()
        .filter_map(|item| item.get("id").and_then(Value::as_u64))
        .map(|id| id as u32)
        .collect()
}

fn add_counts(target: &mut Stats, item: &ItemInfo) {
    target.items += 1;
    if let Some(rarity) = &item.rarity {
        if !rarity.is_empty() {
            *target.by_rarity.entry(rarity.clone()).or_default() += 1;
        }
    }
    if let Some(type_) = &item.type_ {
        if !type_.is_empty() {
            *target.by_type.entry(type_.clone()).or_default() += 1;
        }
    }
    if item.rate_up.is_some() {
        target.rate_up_items += 1;
    }
    if item.rate_up.unwrap_or(0) >= 5 {
        target.rate_up_ssr += 1;
    }
}

fn sum_into_totals(target: &mut Totals, source: &Stats) {
    target.pulls += source.pulls;
    target.items += source.items;
    target.rate_up_items += source.rate_up_items;
    target.rate_up_ssr += source.rate_up_ssr;
    for (key, value) in &source.by_rarity {
        *target.by_rarity.entry(key.clone()).or_default() += value;
    }
    for (key, value) in &source.by_type {
        *target.by_type.entry(key.clone()).or_default() += value;
    }
}

fn sum_into_storage(target: &mut StorageOutput, source: &Stats) {
    target.pulls += source.pulls;
    target.items += source.items;
    target.rate_up_items += source.rate_up_items;
    target.rate_up_ssr += source.rate_up_ssr;
    for (key, value) in &source.by_rarity {
        *target.by_rarity.entry(key.clone()).or_default() += value;
    }
    for (key, value) in &source.by_type {
        *target.by_type.entry(key.clone()).or_default() += value;
    }
}

fn build_pity(info: Option<&GachaInfo>) -> Option<Pity> {
    info.map(|info| Pity {
        total_times: info.total_times.unwrap_or(0),
        gacha_total_times: info.gacha_total_times.unwrap_or(0),
        days_count: info.days_count.unwrap_or(0),
        aup_miss_times: info.aup_miss_times.unwrap_or(0),
        amiss_times: info.a_miss_times.unwrap_or(0),
        aup_guarantee_times: info.aup_guarantee_times.unwrap_or(0),
        received_first_ten_reward: info.reve_first_ten_reward.unwrap_or(false),
        received_guarantee_reward: info.recv_guarantee_reward.unwrap_or(false),
    })
}

fn build_spark(banner: &Banner, stats: &Stats, pity: Option<&Pity>) -> Spark {
    let sparkable =
        banner.type_.as_deref() == Some("character") && !banner.rate_up5_star.is_empty();
    let banner_pulls = pity.map(|pity| pity.gacha_total_times);
    let counted_pulls = if sparkable {
        stats.items.max(banner_pulls.unwrap_or(0) as u64)
    } else {
        stats.items
    };
    Spark {
        sparkable,
        threshold: SPARK_THRESHOLD,
        counted_pulls,
        history_items: stats.items,
        banner_pulls,
        sparked: sparkable && counted_pulls >= SPARK_THRESHOLD,
    }
}

pub fn build_region_output(
    region_key: &str,
    fetched: &FetchedRegion,
    existing: Option<&ExistingRegion>,
    item_data: &Value,
) -> Result<RegionOutput> {
    let existing_pulls: Vec<Pull> = match existing {
        Some(existing) if value_string(&existing.uid) == fetched.uid => existing
            .history
            .iter()
            .map(|entry| Pull {
                banner_id: entry.banner_id,
                storage_id: entry.storage_id,
                time: entry.time,
                ids: entry.ids.clone(),
            })
            .collect(),
        _ => Vec::new(),
    };
    let pulls = merge_pulls(&existing_pulls, &fetched.pulls);

    let mut all_banners = fetched.catalog.banners.clone();
    let mut banner_by_id = fetched.catalog.banner_by_id.clone();
    for pull in &pulls {
        if !banner_by_id.contains_key(&pull.banner_id) {
            let storage = fetched.catalog.storages.get(&pull.storage_id);
            let banner = Banner {
                id: pull.banner_id,
                storage_id: pull.storage_id,
                storage_key: storage.map(|storage| storage.key.clone()),
                storage_name: storage.map(|storage| storage.name.clone()),
                type_: storage.map(|storage| storage.type_.clone()),
                name: format!("Unknown Banner {}", pull.banner_id),
                icon: None,
                image: None,
                start_time: None,
                end_time: None,
                active: false,
                rate_up5_star: Vec::new(),
                rate_up4_star: Vec::new(),
                unknown: Some(true),
            };
            banner_by_id.insert(banner.id, all_banners.len());
            all_banners.push(banner);
        }
    }

    let info_by_id: HashMap<u32, GachaInfo> = fetched
        .gacha_info
        .iter()
        .filter_map(|info| info.id.filter(|id| *id > 0).map(|id| (id, info.clone())))
        .collect();

    let mut items_by_id: BTreeMap<u32, ItemInfo> = BTreeMap::new();
    let mut per_banner: HashMap<u32, Vec<HistoryEntry>> = all_banners
        .iter()
        .map(|banner| (banner.id, Vec::new()))
        .collect();
    let rate_up5: HashMap<u32, HashSet<u32>> = all_banners
        .iter()
        .map(|banner| (banner.id, rate_up_ids(&banner.rate_up5_star)))
        .collect();
    let rate_up4: HashMap<u32, HashSet<u32>> = all_banners
        .iter()
        .map(|banner| (banner.id, rate_up_ids(&banner.rate_up4_star)))
        .collect();

    let mut get_item = |id: u32, banner_id: u32| -> ItemInfo {
        let key = id.to_string();
        let raw = item_data.get(key.as_str());
        let rate_up = if rate_up5
            .get(&banner_id)
            .map(|ids| ids.contains(&id))
            .unwrap_or(false)
        {
            Some(5)
        } else if rate_up4
            .get(&banner_id)
            .map(|ids| ids.contains(&id))
            .unwrap_or(false)
        {
            Some(4)
        } else {
            None
        };
        let item = ItemInfo {
            id,
            name: raw
                .and_then(|raw| raw.get("name"))
                .and_then(Value::as_str)
                .map(str::to_string),
            type_: raw
                .and_then(|raw| raw.get("type"))
                .and_then(Value::as_str)
                .map(str::to_string),
            rarity: raw
                .and_then(|raw| raw.get("rarity"))
                .and_then(Value::as_str)
                .map(str::to_string),
            rate_up,
        };

        match items_by_id.get_mut(&id) {
            None => {
                items_by_id.insert(id, item.clone());
            }
            Some(existing) => {
                if item.rate_up.is_some() && existing.rate_up.is_none() {
                    existing.rate_up = item.rate_up;
                }
            }
        }
        item
    };

    for pull in &pulls {
        let items: Vec<ItemInfo> = pull
            .ids
            .iter()
            .map(|id| get_item(*id, pull.banner_id))
            .collect();
        let entry = HistoryEntry {
            time: pull.time,
            time_iso: iso_from_unix(pull.time),
            ids: pull.ids.clone(),
            items,
        };
        if let Some(entries) = per_banner.get_mut(&pull.banner_id) {
            entries.push(entry);
        }
    }

    let mut totals = Totals {
        pulls: 0,
        items: 0,
        rate_up_items: 0,
        rate_up_ssr: 0,
        by_rarity: BTreeMap::new(),
        by_type: BTreeMap::new(),
        banners: 0,
        active_banners: 0,
        banners_with_history: 0,
        sparks: 0,
        sparked_banner_ids: Vec::new(),
    };

    let mut storage_stats: BTreeMap<u32, StorageOutput> = fetched
        .catalog
        .storages
        .values()
        .map(|storage| {
            (
                storage.id,
                StorageOutput {
                    id: storage.id,
                    key: storage.key.clone(),
                    name: storage.name.clone(),
                    type_: storage.type_.clone(),
                    banner_ids: Vec::new(),
                    pulls: 0,
                    items: 0,
                    rate_up_items: 0,
                    rate_up_ssr: 0,
                    by_rarity: BTreeMap::new(),
                    by_type: BTreeMap::new(),
                },
            )
        })
        .collect();

    let mut output_banners: Vec<BannerOutput> = Vec::with_capacity(all_banners.len());

    for banner in &all_banners {
        let mut history = per_banner.remove(&banner.id).unwrap_or_default();
        history.sort_by(|a, b| b.time.cmp(&a.time));

        let mut stats = Stats::default();
        for entry in &history {
            stats.pulls += if !entry.items.is_empty() {
                entry.items.len() as u64
            } else if !entry.ids.is_empty() {
                entry.ids.len() as u64
            } else {
                1
            };
            for item in &entry.items {
                add_counts(&mut stats, item);
            }
        }
        if let (Some(first), Some(last)) = (history.last(), history.first()) {
            stats.first_pull_time = Some(first.time);
            stats.last_pull_time = Some(last.time);
        }

        let pity = build_pity(info_by_id.get(&banner.id));
        let spark = build_spark(banner, &stats, pity.as_ref());

        output_banners.push(BannerOutput {
            id: banner.id,
            storage_id: banner.storage_id,
            storage_key: banner.storage_key.clone(),
            storage_name: banner.storage_name.clone(),
            type_: banner.type_.clone(),
            name: banner.name.clone(),
            icon: banner.icon.clone(),
            image: banner.image.clone(),
            start_time: banner.start_time.clone(),
            end_time: banner.end_time.clone(),
            active: banner.active,
            rate_up5_star: banner.rate_up5_star.clone(),
            rate_up4_star: banner.rate_up4_star.clone(),
            unknown: banner.unknown,
            stats,
            pity,
            spark,
            history,
        });

        let stats = &output_banners.last().unwrap().stats;
        sum_into_totals(&mut totals, stats);
        if let Some(storage) = storage_stats.get_mut(&banner.storage_id) {
            storage.banner_ids.push(banner.id);
            sum_into_storage(storage, stats);
        }
    }

    for storage in storage_stats.values_mut() {
        storage.banner_ids.sort_unstable();
    }

    totals.banners = output_banners.len();
    totals.active_banners = output_banners.iter().filter(|banner| banner.active).count();
    totals.banners_with_history = output_banners
        .iter()
        .filter(|banner| !banner.history.is_empty())
        .count();
    totals.sparks = output_banners
        .iter()
        .filter(|banner| banner.spark.sparked)
        .count();
    totals.sparked_banner_ids = output_banners
        .iter()
        .filter(|banner| banner.spark.sparked)
        .map(|banner| banner.id)
        .collect();

    let mut flat_history: Vec<FlatHistoryEntry> = pulls
        .iter()
        .filter(|pull| pull.time != 0)
        .map(|pull| FlatHistoryEntry {
            banner_id: pull.banner_id,
            banner_name: banner_by_id
                .get(&pull.banner_id)
                .and_then(|index| all_banners.get(*index))
                .map(|banner| banner.name.clone()),
            storage_id: pull.storage_id,
            time: pull.time,
            time_iso: iso_from_unix(pull.time),
            count: pull.ids.len(),
            ids: pull.ids.clone(),
        })
        .collect();
    flat_history.sort_by(|a, b| b.time.cmp(&a.time).then(a.banner_id.cmp(&b.banner_id)));

    output_banners.sort_by(|a, b| a.storage_id.cmp(&b.storage_id).then(a.id.cmp(&b.id)));

    Ok(RegionOutput {
        region: region_key.to_string(),
        uid: fetched.uid.clone(),
        updated_at: Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true),
        totals,
        storages: storage_stats.into_values().collect(),
        banners: output_banners,
        items: items_by_id,
        history: flat_history,
        errors: fetched.errors.clone(),
    })
}

pub fn output_file_for_region(region_key: &str) -> Result<PathBuf> {
    Ok(std::env::current_dir()?.join(format!("gacha.{region_key}.json")))
}

pub fn load_existing_region(path: &Path) -> Option<ExistingRegion> {
    if !path.exists() {
        return None;
    }
    let text = fs::read_to_string(path).ok()?;
    match serde_json::from_str(&text) {
        Ok(region) => Some(region),
        Err(error) => {
            eprintln!(
                "Could not read existing {}: {error}",
                path.file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("region file")
            );
            None
        }
    }
}

fn pretty_json<T: Serialize>(value: &T) -> Result<String> {
    let mut buffer = Vec::new();
    let formatter = serde_json::ser::PrettyFormatter::with_indent(b"    ");
    let mut serializer = serde_json::Serializer::with_formatter(&mut buffer, formatter);
    value.serialize(&mut serializer)?;
    Ok(String::from_utf8(buffer)?)
}

pub fn write_region_output(path: &Path, output: &RegionOutput) -> Result<()> {
    let serialized = pretty_json(output)?;
    fs::write(path, serialized).with_context(|| format!("writing {}", path.display()))?;
    Ok(())
}
