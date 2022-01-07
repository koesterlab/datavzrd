use crate::utils::column_type::{classify_table, ColumnType};
use anyhow::Result;
use csv::Reader;
use itertools::Itertools;
use ndhistogram::axis::Uniform;
use ndhistogram::{ndhistogram, Histogram};
use serde::Serialize;
use serde_json::json;
use std::collections::HashMap;
use std::fs;
use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::str::FromStr;
use tera::{Context, Tera};

pub(crate) fn render_plots<P: AsRef<Path>>(
    output_path: P,
    csv_path: &Path,
    separator: char,
) -> Result<()> {
    let column_types = classify_table(csv_path, separator)?;

    let mut reader = csv::ReaderBuilder::new()
        .delimiter(separator as u8)
        .from_path(csv_path)?;

    let path = Path::new(output_path.as_ref()).join("plots");
    fs::create_dir(&path)?;

    for (index, column) in reader.headers()?.iter().enumerate() {
        let mut templates = Tera::default();
        let mut context = Context::new();
        context.insert("title", &column);
        context.insert("index", &index);
        match column_types.get(column) {
            None | Some(ColumnType::None) => unreachable!(),
            Some(ColumnType::String) => {
                let plot = generate_nominal_plot(csv_path, separator, index)?;
                templates.add_raw_template(
                    "plot.js.tera",
                    include_str!("../../../templates/nominal_plot.js.tera"),
                )?;
                context.insert("table", &json!(plot).to_string())
            }
            Some(ColumnType::Integer) | Some(ColumnType::Float) => {
                let plot = generate_numeric_plot(csv_path, separator, index)?;
                templates.add_raw_template(
                    "plot.js.tera",
                    include_str!("../../../templates/numeric_plot.js.tera"),
                )?;
                context.insert("table", &json!(plot).to_string())
            }
        };
        let js = templates.render("plot.js.tera", &context)?;
        let file_path = path.join(Path::new(&format!("plot_{}", index)).with_extension("js"));
        let mut file = fs::File::create(file_path)?;
        file.write_all(js.as_bytes())?;
    }
    Ok(())
}

fn generate_numeric_plot(
    path: &Path,
    separator: char,
    column_index: usize,
) -> Result<Vec<BinnedPlotRecord>> {
    let generate_reader = || -> csv::Result<Reader<File>> {
        csv::ReaderBuilder::new()
            .delimiter(separator as u8)
            .from_path(&path)
    };

    let mut min_reader = generate_reader()?;
    let mut max_reader = generate_reader()?;
    let mut reader = generate_reader()?;

    let min = min_reader
        .records()
        .map(|r| f32::from_str(r.unwrap().get(column_index).unwrap()).unwrap())
        .fold(f32::INFINITY, |a, b| a.min(b));
    let max = max_reader
        .records()
        .map(|r| f32::from_str(r.unwrap().get(column_index).unwrap()).unwrap())
        .fold(f32::NEG_INFINITY, |a, b| a.max(b));

    let mut hist = ndhistogram!(Uniform::new(NUMERIC_BINS, min, max));
    let mut nan = 0;

    for r in reader.records() {
        let record = r?;
        let value = record.get(column_index).unwrap();
        if let Ok(number) = f32::from_str(value) {
            hist.fill(&number)
        } else {
            nan += 1;
        }
    }

    let mut result = hist
        .iter()
        .map(|h| BinnedPlotRecord::new(h.bin.start(), h.bin.end(), *h.value))
        .collect_vec();

    if nan > 0 {
        result.push(BinnedPlotRecord {
            bin_start: f32::NAN,
            bin_end: f32::NAN,
            value: nan,
        })
    }

    Ok(result)
}

fn generate_nominal_plot(
    path: &Path,
    separator: char,
    column_index: usize,
) -> Result<Option<Vec<PlotRecord>>> {
    let mut reader = csv::ReaderBuilder::new()
        .delimiter(separator as u8)
        .from_path(path)?;

    let mut count_values = HashMap::new();

    for record in reader.records() {
        let result = record?;
        let value = result.get(column_index).unwrap();
        if !value.is_empty() {
            let entry = count_values.entry(value.to_owned()).or_insert_with(|| 0);
            *entry += 1;
        }
    }

    let mut plot_data = count_values
        .iter()
        .map(|(k, v)| PlotRecord {
            key: k.to_string(),
            value: *v,
        })
        .collect_vec();

    if plot_data.len() > MAX_NOMINAL_BINS {
        let unique_values = count_values.iter().map(|(_, v)| v).unique().count();
        if unique_values <= 1 {
            return Ok(None);
        };
        plot_data.sort_by(|a, b| b.value.cmp(&a.value));
        plot_data = plot_data.into_iter().take(MAX_NOMINAL_BINS).collect();
    }

    Ok(Some(plot_data))
}

const MAX_NOMINAL_BINS: usize = 10;
const NUMERIC_BINS: usize = 20;

#[derive(Serialize, Debug, Clone, Ord, PartialOrd, Eq, PartialEq)]
struct PlotRecord {
    key: String,
    value: u32,
}

#[derive(Serialize, Debug, Clone, PartialEq)]
struct BinnedPlotRecord {
    bin_start: f32,
    bin_end: f32,
    value: u32,
}

impl BinnedPlotRecord {
    fn new(start: Option<f32>, end: Option<f32>, value: f64) -> BinnedPlotRecord {
        BinnedPlotRecord {
            bin_start: start.unwrap_or_else(|| f32::NEG_INFINITY),
            bin_end: end.unwrap_or_else(|| f32::INFINITY),
            value: value as u32,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::render::portable::plot::{generate_nominal_plot, PlotRecord};

    #[test]
    fn test_nominal_plot_generation() {
        let mut records =
            generate_nominal_plot("tests/data/uniform_datatypes.csv".as_ref(), ',', 0)
                .unwrap()
                .unwrap();
        let mut expected = vec![
            PlotRecord {
                key: String::from("George"),
                value: 2,
            },
            PlotRecord {
                key: String::from("Delia"),
                value: 1,
            },
            PlotRecord {
                key: String::from("Winnie"),
                value: 1,
            },
        ];
        assert_eq!(records.sort(), expected.sort());
    }
}