mod plot;
pub(crate) mod utils;

use crate::render::portable::plot::get_min_max;
use crate::render::portable::plot::render_plots;
use crate::render::Renderer;
use crate::spec::{
    CustomPlot, DatasetSpecs, Heatmap, ItemSpecs, ItemsSpec, LinkSpec, RenderColumnSpec, TickPlot,
};
use crate::utils::column_index::ColumnIndex;
use crate::utils::column_type::{classify_table, ColumnType};
use crate::utils::row_address::RowAddressFactory;
use anyhow::Result;
use anyhow::{bail, Context as AnyhowContext};
use chrono::{DateTime, Local};
use csv::{Reader, StringRecord};
use itertools::Itertools;
use lz_str::compress_to_utf16;
use serde::Serialize;
use serde_json::json;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::fs::File;
use std::io::Write;
use std::option::Option::Some;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use tera::{Context, Tera};
use thiserror::Error;
use typed_builder::TypedBuilder;

#[derive(TypedBuilder, Debug)]
pub(crate) struct ItemRenderer {
    specs: ItemsSpec,
}

type LinkedTable = HashMap<(String, String), ColumnIndex>;

impl Renderer for ItemRenderer {
    /// Render all items of user config
    fn render_tables<P>(&self, path: P) -> Result<()>
    where
        P: AsRef<Path>,
    {
        for (name, table) in &self.specs.views {
            let out_path = Path::new(path.as_ref()).join(name);
            fs::create_dir(&out_path)?;
            if table.render_plot.is_some() {
                if let Some(datasets) = &table.datasets {
                    // Render plot with multiple datasets
                    render_plot_page_with_multiple_datasets(
                        &out_path,
                        &self.specs.views.keys().map(|s| s.to_owned()).collect_vec(),
                        name,
                        table,
                        datasets
                            .iter()
                            .map(|(n, name)| {
                                (n.to_string(), self.specs.datasets.get(name).unwrap())
                            })
                            .collect(),
                        &self.specs.views,
                        &self.specs.default_view,
                    )?;
                    continue;
                }
            }
            let dataset = match self.specs.datasets.get(table.dataset.as_ref().unwrap()) {
                Some(dataset) => dataset,
                None => {
                    bail!(DatasetError::NotFound {
                        dataset_name: table.dataset.as_ref().unwrap().clone()
                    })
                }
            };

            let generate_reader = || -> csv::Result<Reader<File>> {
                csv::ReaderBuilder::new()
                    .delimiter(dataset.separator as u8)
                    .from_path(&dataset.path)
            };

            let mut counter_reader = generate_reader()
                .context(format!("Could not read file with path {:?}", &dataset.path))?;
            let records_length = counter_reader.records().count();
            if records_length > 0 {
                let linked_tables = get_linked_tables(name, &self.specs)?;
                // Render plot
                if table.render_plot.is_some() {
                    render_plot_page(
                        &out_path,
                        &self.specs.views.keys().map(|s| s.to_owned()).collect_vec(),
                        name,
                        table,
                        dataset,
                        &linked_tables,
                        dataset.links.as_ref().unwrap(),
                        &self.specs.views,
                        &self.specs.default_view,
                    )?;
                }
                // Render table
                else if let Some(table_specs) = &table.render_table {
                    let row_address_factory = RowAddressFactory::new(table.page_size);
                    let pages = row_address_factory
                        .get(records_length - dataset.header_rows)
                        .page
                        + 1;

                    let is_single_page = if let Some(max_rows) = table.max_in_memory_rows {
                        records_length <= max_rows
                    } else {
                        records_length <= self.specs.max_in_memory_rows
                    };

                    let mut reader = generate_reader()
                        .context(format!("Could not read file with path {:?}", &dataset.path))?;
                    let headers = reader.headers()?.iter().map(|s| s.to_owned()).collect_vec();

                    let table_specs = &table_specs
                        .clone()
                        .into_iter()
                        .filter(|(k, s)| !s.optional || headers.contains(k))
                        .collect();

                    let additional_headers = if dataset.header_rows > 1 {
                        let mut additional_header_reader = generate_reader().context(format!(
                            "Could not read file with path {:?}",
                            &dataset.path
                        ))?;
                        Some(
                            additional_header_reader
                                .records()
                                .take(dataset.header_rows - 1)
                                .map(|r| r.unwrap())
                                .collect_vec(),
                        )
                    } else {
                        None
                    };

                    for (page, grouped_records) in &reader
                        .records()
                        .skip(dataset.header_rows - 1)
                        .into_iter()
                        .enumerate()
                        .group_by(|(i, _)| row_address_factory.get(*i).page)
                    {
                        let records = grouped_records.collect_vec();
                        render_page(
                            &out_path,
                            page + 1,
                            pages,
                            records
                                .iter()
                                .map(|(_, records)| records.as_ref().unwrap())
                                .collect_vec(),
                            &headers,
                            &self.specs.views.keys().map(|s| s.to_owned()).collect_vec(),
                            name,
                            table.description.as_deref(),
                            &linked_tables,
                            dataset.links.as_ref().unwrap(),
                            &self.specs.report_name,
                            &self.specs.views,
                            &self.specs.default_view,
                        )?;
                    }
                    render_table_javascript(
                        &out_path,
                        &headers,
                        &dataset.path,
                        dataset.separator,
                        table_specs,
                        additional_headers,
                        is_single_page,
                    )?;
                    render_plots(
                        &out_path,
                        &dataset.path,
                        dataset.separator,
                        dataset.header_rows,
                    )?;
                    render_search_dialogs(
                        &out_path,
                        &headers,
                        &dataset.path,
                        dataset.separator,
                        table.page_size,
                        dataset.header_rows,
                    )?;
                }
            } else {
                render_empty_dataset(
                    &out_path,
                    name,
                    &self.specs.report_name,
                    &self.specs.views.keys().map(|s| s.to_owned()).collect_vec(),
                )?;
            }
        }
        Ok(())
    }
}

#[allow(clippy::too_many_arguments)]
/// Render single page of a table
fn render_page<P: AsRef<Path>>(
    output_path: P,
    page_index: usize,
    pages: usize,
    data: Vec<&StringRecord>,
    titles: &[String],
    tables: &[String],
    name: &str,
    description: Option<&str>,
    linked_tables: &LinkedTable,
    links: &HashMap<String, LinkSpec>,
    report_name: &str,
    views: &HashMap<String, ItemSpecs>,
    default_view: &Option<String>,
) -> Result<()> {
    let mut templates = Tera::default();
    templates.add_raw_template(
        "table.html.tera",
        include_str!("../../../templates/table.html.tera"),
    )?;
    let mut context = Context::new();

    let data = data
        .iter()
        .map(|s| s.iter().map(|s| s.to_string()).collect_vec())
        .collect_vec();

    let compressed_linkouts = if !links.is_empty() {
        let linkouts = data
            .iter()
            .map(|r| render_link_column(r, linked_tables, titles, links).unwrap())
            .collect_vec();
        Some(compress_to_utf16(&json!(linkouts).to_string()))
    } else {
        None
    };

    let compressed_data = compress_to_utf16(&json!(data).to_string());

    let local: DateTime<Local> = Local::now();

    context.insert("data", &json!(compressed_data).to_string());
    context.insert("linkouts", &json!(compressed_linkouts).to_string());
    context.insert("titles", &titles.iter().collect_vec());
    context.insert("current_page", &page_index);
    context.insert("pages", &pages);
    context.insert("description", &description);
    context.insert(
        "tables",
        &tables
            .iter()
            .filter(|t| !views.get(*t).unwrap().hidden)
            .filter(|t| {
                if let Some(default_view) = default_view {
                    t != &default_view
                } else {
                    true
                }
            })
            .collect_vec(),
    );
    context.insert("default_view", default_view);
    context.insert("name", name);
    context.insert("report_name", report_name);
    context.insert("time", &local.format("%a %b %e %T %Y").to_string());
    context.insert("version", &env!("CARGO_PKG_VERSION"));

    let file_path = Path::new(output_path.as_ref())
        .join(Path::new(&format!("index_{}", page_index)).with_extension("html"));

    let html = templates.render("table.html.tera", &context)?;

    let mut file = fs::File::create(file_path)?;
    file.write_all(html.as_bytes())?;

    Ok(())
}

/// Render javascript files for each table containing formatters
fn render_table_javascript<P: AsRef<Path>>(
    output_path: P,
    titles: &[String],
    csv_path: &Path,
    separator: char,
    render_columns: &HashMap<String, RenderColumnSpec>,
    additional_headers: Option<Vec<StringRecord>>,
    is_single_page: bool,
) -> Result<()> {
    let mut templates = Tera::default();
    templates.add_raw_template(
        "table.js.tera",
        include_str!("../../../templates/table.js.tera"),
    )?;
    let mut context = Context::new();

    let header_row_length = additional_headers
        .clone()
        .unwrap_or_else(|| vec![StringRecord::from(vec![""])])
        .len();

    let formatters: HashMap<String, String> = render_columns
        .iter()
        .filter(|(_, k)| k.custom.is_some())
        .map(|(k, v)| (k.to_owned(), v.custom.as_ref().unwrap().to_owned()))
        .collect();

    let numeric: HashMap<String, bool> = classify_table(csv_path, separator)?
        .iter()
        .map(|(k, v)| (k.to_owned(), *v != ColumnType::String))
        .collect();

    let custom_plots: HashMap<String, CustomPlot> = render_columns
        .iter()
        .filter(|(_, k)| k.custom_plot.is_some())
        .map(|(k, v)| {
            let mut custom_plot = v.custom_plot.as_ref().unwrap().to_owned();
            custom_plot.read_schema().unwrap();
            (k.to_owned(), custom_plot)
        })
        .collect();

    let tick_plots: HashMap<String, String> = render_columns
        .iter()
        .filter(|(_, k)| k.plot.is_some())
        .filter(|(_, k)| k.plot.as_ref().unwrap().tick_plot.is_some())
        .map(|(k, v)| {
            (
                k.to_owned(),
                render_tick_plot(
                    k,
                    csv_path,
                    separator,
                    header_row_length,
                    v.plot.as_ref().unwrap().tick_plot.as_ref().unwrap(),
                )
                .unwrap(),
            )
        })
        .collect();

    let brush_domains: HashMap<String, Vec<f32>> = render_columns
        .iter()
        .filter_map(|(title, k)| k.plot.as_ref().map(|plot| (title, plot)))
        .filter_map(|(title, k)| k.tick_plot.as_ref().map(|tick_plot| (title, tick_plot)))
        .filter_map(|(title, k)| k.domain.as_ref().map(|domain| (title, domain)))
        .map(|(title, k)| (title.to_string(), k.to_vec()))
        .chain(
            render_columns
                .iter()
                .filter(|(title, _)| *numeric.get(&title.to_string()).unwrap_or(&false))
                .filter_map(|(title, k)| k.plot.as_ref().map(|plot| (title, plot)))
                .filter_map(|(title, k)| k.heatmap.as_ref().map(|heatmap| (title, heatmap)))
                .filter_map(|(title, k)| k.domain.as_ref().map(|domain| (title, domain)))
                .map(|(title, k)| {
                    (
                        title.to_string(),
                        k.iter().map(|s| f32::from_str(s).unwrap()).collect_vec(),
                    )
                }),
        )
        .collect();

    let aux_domains: HashMap<String, Vec<_>> = render_columns
        .iter()
        .filter_map(|(title, k)| k.plot.as_ref().map(|plot| (title, plot)))
        .filter_map(|(title, k)| k.tick_plot.as_ref().map(|tick_plot| (title, tick_plot)))
        .filter_map(|(title, k)| {
            k.aux_domain_columns
                .0
                .as_ref()
                .map(|domains| (title, domains))
        })
        .map(|(title, k)| (title.to_string(), k.to_vec()))
        .chain(
            render_columns
                .iter()
                .filter_map(|(title, k)| k.plot.as_ref().map(|plot| (title, plot)))
                .filter_map(|(title, k)| k.heatmap.as_ref().map(|heatmap| (title, heatmap)))
                .filter_map(|(title, k)| {
                    k.aux_domain_columns
                        .0
                        .as_ref()
                        .map(|domains| (title, domains))
                })
                .map(|(title, k)| (title.to_string(), k.to_vec())),
        )
        .collect();

    let heatmaps: HashMap<String, (&Heatmap, String)> = render_columns
        .iter()
        .filter(|(_, k)| k.plot.is_some())
        .filter(|(_, k)| k.plot.as_ref().unwrap().heatmap.is_some())
        .map(|(k, v)| {
            (
                k.to_owned(),
                (
                    v.plot.as_ref().unwrap().heatmap.as_ref().unwrap(),
                    get_column_domain(
                        k,
                        csv_path,
                        separator,
                        header_row_length,
                        v.plot.as_ref().unwrap().heatmap.as_ref().unwrap(),
                    )
                    .unwrap(),
                ),
            )
        })
        .collect();

    let link_urls: HashMap<String, String> = render_columns
        .iter()
        .filter(|(_, k)| k.link_to_url.is_some())
        .map(|(t, spec)| {
            (
                t.to_string(),
                spec.link_to_url.as_ref().unwrap().to_string(),
            )
        })
        .collect();

    let ellipses: HashMap<String, u32> = render_columns
        .iter()
        .filter(|(_, k)| k.ellipsis.is_some())
        .map(|(t, spec)| (t.to_string(), spec.ellipsis.unwrap()))
        .collect();

    let mut display_modes: HashMap<String, String> = titles
        .iter()
        .map(|t| (t.to_string(), "normal".to_string()))
        .collect();
    for (title, rc) in render_columns {
        display_modes.insert(title.to_string(), rc.display_mode.to_string());
    }
    let detail_mode = display_modes
        .iter()
        .filter(|(_, mode)| *mode != "normal")
        .count()
        > 0;

    let header_rows = additional_headers.map(|headers| {
        headers
            .iter()
            .map(|r| {
                r.iter()
                    .enumerate()
                    .map(|(i, v)| (&titles[i], v.to_string()))
                    .collect::<HashMap<_, _>>()
            })
            .collect_vec()
    });

    context.insert("titles", &titles.iter().collect_vec());
    context.insert("additional_headers", &header_rows);
    context.insert("formatter", &Some(formatters));
    context.insert("custom_plots", &custom_plots);
    context.insert("tick_plots", &tick_plots);
    context.insert("heatmaps", &heatmaps);
    context.insert("ellipses", &ellipses);
    context.insert("display_modes", &display_modes);
    context.insert("detail_mode", &detail_mode);
    context.insert("link_urls", &link_urls);
    context.insert("num", &numeric);
    context.insert("brush_domains", &json!(brush_domains).to_string());
    context.insert("aux_domains", &json!(aux_domains).to_string());
    context.insert("is_single_page", &is_single_page);

    let file_path = Path::new(output_path.as_ref()).join(Path::new("table").with_extension("js"));

    let js = templates.render("table.js.tera", &context)?;

    let mut file = fs::File::create(file_path)?;
    file.write_all(js.as_bytes())?;

    Ok(())
}

/// Renders an empty page when datasets are empty
fn render_empty_dataset<P: AsRef<Path>>(
    output_path: P,
    name: &str,
    report_name: &str,
    tables: &[String],
) -> Result<()> {
    let mut templates = Tera::default();
    templates.add_raw_template(
        "empty.html.tera",
        include_str!("../../../templates/empty.html.tera"),
    )?;
    let mut context = Context::new();
    let local: DateTime<Local> = Local::now();

    context.insert("tables", tables);
    context.insert("name", name);
    context.insert("report_name", report_name);
    context.insert("time", &local.format("%a %b %e %T %Y").to_string());
    context.insert("version", &env!("CARGO_PKG_VERSION"));

    let file_path =
        Path::new(output_path.as_ref()).join(Path::new("index_1").with_extension("html"));

    let html = templates.render("empty.html.tera", &context)?;

    let mut file = fs::File::create(file_path)?;
    file.write_all(html.as_bytes())?;

    Ok(())
}

/// Renders search dialog modals for individual columns of every table
fn render_search_dialogs<P: AsRef<Path>>(
    path: P,
    titles: &[String],
    csv_path: &Path,
    separator: char,
    page_size: usize,
    header_rows: usize,
) -> Result<()> {
    let output_path = Path::new(path.as_ref()).join("search");
    fs::create_dir(&output_path)?;
    for (column, title) in titles.iter().enumerate() {
        let mut reader = csv::ReaderBuilder::new()
            .delimiter(separator as u8)
            .from_path(csv_path)
            .context(format!("Could not read file with path {:?}", csv_path))?;

        let row_address_factory = RowAddressFactory::new(page_size);

        let records = &reader
            .records()
            .skip(header_rows - 1)
            .map(|row| row.unwrap())
            .map(|row| row.get(column).unwrap().to_string())
            .enumerate()
            .map(|(i, row)| (row, row_address_factory.get(i)))
            .map(|(row, address)| (row, address.page + 1, address.row))
            .collect_vec();

        let mut templates = Tera::default();
        templates.add_raw_template(
            "search_dialog.html.tera",
            include_str!("../../../templates/search_dialog.html.tera"),
        )?;
        let mut context = Context::new();
        context.insert("records", &records);
        context.insert("title", &title);

        let file_path = Path::new(&output_path)
            .join(Path::new(&format!("column_{}", column)).with_extension("html"));

        let html = templates.render("search_dialog.html.tera", &context)?;

        let mut file = fs::File::create(file_path)?;
        file.write_all(html.as_bytes())?;
    }
    Ok(())
}

fn get_linked_tables(table: &str, specs: &ItemsSpec) -> Result<LinkedTable> {
    let table_spec = specs.views.get(table).unwrap();
    let dataset = &specs
        .datasets
        .get(table_spec.dataset.as_ref().unwrap())
        .unwrap();
    let links = &dataset
        .links
        .as_ref()
        .unwrap()
        .iter()
        .filter_map(|(_, link_spec)| link_spec.table_row.as_ref())
        .map(|link| link.split_once('/').unwrap())
        .collect_vec();

    let mut result = HashMap::new();

    for (table, column) in links {
        let linked_table = &specs.views.get(*table).unwrap();
        let other_dataset = match specs.datasets.get(linked_table.dataset.as_ref().unwrap()) {
            Some(dataset) => dataset,
            None => {
                bail!(DatasetError::NotFound {
                    dataset_name: table_spec.dataset.as_ref().unwrap().clone()
                })
            }
        };
        let page_size = specs.views.get(*table).unwrap().page_size;

        let column_index = ColumnIndex::new(
            &other_dataset.path,
            other_dataset.separator,
            column,
            page_size,
        )?;

        result.insert((table.to_string(), column.to_string()), column_index);
    }
    Ok(result)
}

/// Renders tick plots for given csv column
fn render_tick_plot(
    title: &str,
    csv_path: &Path,
    separator: char,
    header_rows: usize,
    tick_plot: &TickPlot,
) -> Result<String> {
    let mut reader = csv::ReaderBuilder::new()
        .delimiter(separator as u8)
        .from_path(csv_path)
        .context(format!("Could not read file with path {:?}", csv_path))?;

    let column_index = reader.headers().map(|s| {
        s.iter()
            .position(|t| t == title)
            .context(ColumnError::NotFound {
                column: title.to_string(),
                path: csv_path.to_str().unwrap().to_string(),
            })
            .unwrap()
    })?;

    let (min, max) = if let Some(domain) = &tick_plot.domain {
        (domain[0], domain[1])
    } else if let Some(aux_domain_columns) = &tick_plot.aux_domain_columns.0 {
        let columns = aux_domain_columns
            .iter()
            .map(|s| s.to_string())
            .chain(vec![title.to_string()].into_iter())
            .collect();
        get_min_max_multiple_columns(csv_path, separator, header_rows, columns)?
    } else {
        get_min_max(csv_path, separator, column_index, header_rows)?
    };

    let mut templates = Tera::default();
    templates.add_raw_template(
        "tick_plot.vl.tera",
        include_str!("../../../templates/tick_plot.vl.tera"),
    )?;
    let mut context = Context::new();
    context.insert("minimum", &min);
    context.insert("maximum", &max);
    context.insert("field", &title);
    context.insert("scale_type", &tick_plot.scale_type);

    Ok(templates.render("tick_plot.vl.tera", &context)?)
}

fn get_column_domain(
    title: &str,
    csv_path: &Path,
    separator: char,
    header_rows: usize,
    heatmap: &Heatmap,
) -> Result<String> {
    if let Some(domain) = &heatmap.domain {
        return Ok(json!(domain).to_string());
    }

    let mut reader = csv::ReaderBuilder::new()
        .delimiter(separator as u8)
        .from_path(csv_path)
        .context(format!("Could not read file with path {:?}", csv_path))?;

    let column_index = reader
        .headers()
        .map(|s| s.iter().position(|t| t == title).unwrap())?;

    match heatmap.scale_type.as_str() {
        "ordinal" => {
            if let Some(aux_domain_columns) = &heatmap.aux_domain_columns.0 {
                let columns = aux_domain_columns
                    .iter()
                    .map(|s| s.to_string())
                    .chain(vec![title.to_string()].into_iter())
                    .collect_vec();
                let column_indexes: HashSet<_> = reader.headers().map(|s| {
                    s.iter()
                        .enumerate()
                        .filter(|(_, title)| columns.contains(&title.to_string()))
                        .map(|(index, _)| index)
                        .collect()
                })?;
                Ok(json!(reader
                    .records()
                    .map(|r| r.unwrap())
                    .flat_map(|r| r
                        .iter()
                        .enumerate()
                        .filter(|(index, _)| column_indexes.contains(index))
                        .map(|(_, value)| value.to_string())
                        .collect_vec())
                    .unique()
                    .collect_vec())
                .to_string())
            } else {
                Ok(json!(reader
                    .records()
                    .map(|r| r.unwrap())
                    .map(|r| r.get(column_index).unwrap().to_owned())
                    .unique()
                    .collect_vec())
                .to_string())
            }
        }
        _ => {
            if let Some(aux_domain_columns) = &heatmap.aux_domain_columns.0 {
                let columns = aux_domain_columns
                    .iter()
                    .map(|s| s.to_string())
                    .chain(vec![title.to_string()].into_iter())
                    .collect();
                Ok(json!(get_min_max_multiple_columns(
                    csv_path,
                    separator,
                    header_rows,
                    columns
                )?)
                .to_string())
            } else {
                Ok(json!(get_min_max(csv_path, separator, column_index, header_rows)?).to_string())
            }
        }
    }
}

/// Returns the minimum and maximum for multiple given columns
fn get_min_max_multiple_columns(
    csv_path: &Path,
    separator: char,
    header_rows: usize,
    columns: Vec<String>,
) -> Result<(f32, f32)> {
    let mut mins = Vec::new();
    let mut maxs = Vec::new();
    for column in columns {
        let mut reader = csv::ReaderBuilder::new()
            .delimiter(separator as u8)
            .from_path(csv_path)
            .context(format!("Could not read file with path {:?}", csv_path))?;
        let column_index = reader.headers().map(|s| {
            s.iter()
                .position(|t| t == column)
                .context(ColumnError::NotFound {
                    column: column.to_string(),
                    path: csv_path.to_str().unwrap().to_string(),
                })
                .unwrap()
        })?;
        let (min, max) = get_min_max(csv_path, separator, column_index, header_rows)?;
        mins.push(min);
        maxs.push(max);
    }
    Ok((
        mins.into_iter().reduce(f32::min).unwrap(),
        maxs.into_iter().reduce(f32::max).unwrap(),
    ))
}

#[allow(clippy::too_many_arguments)]
/// Renders a plot page from given render-plot spec
fn render_plot_page<P: AsRef<Path>>(
    output_path: P,
    tables: &[String],
    name: &str,
    item_spec: &ItemSpecs,
    dataset: &DatasetSpecs,
    linked_tables: &LinkedTable,
    links: &HashMap<String, LinkSpec>,
    views: &HashMap<String, ItemSpecs>,
    default_view: &Option<String>,
) -> Result<()> {
    let generate_reader = || -> csv::Result<Reader<File>> {
        csv::ReaderBuilder::new()
            .delimiter(dataset.separator as u8)
            .from_path(&dataset.path)
    };
    let mut reader =
        generate_reader().context(format!("Could not read file with path {:?}", &dataset.path))?;

    let headers = reader.headers()?.iter().map(|s| s.to_owned()).collect_vec();
    let mut records: Vec<HashMap<String, String>> = reader
        .records()
        .skip(&dataset.header_rows - 1)
        .map(|row| {
            row.unwrap()
                .iter()
                .enumerate()
                .map(|(index, record)| (headers.get(index).unwrap().to_owned(), record.to_owned()))
                .collect()
        })
        .collect_vec();

    if !links.is_empty() {
        let mut linkout_reader = generate_reader()
            .context(format!("Could not read file with path {:?}", &dataset.path))?;
        let linkouts = linkout_reader
            .records()
            .map(|row| {
                render_linkouts(
                    &row.unwrap().iter().map(|s| s.to_owned()).collect_vec(),
                    linked_tables,
                    &headers,
                    links,
                )
                .unwrap()
            })
            .collect_vec();

        assert_eq!(records.len(), linkouts.len());

        for (i, record) in records.iter_mut().enumerate() {
            for linkout in &linkouts[i] {
                record.insert(linkout.name.to_string(), linkout.url.to_string());
            }
        }
    }

    let mut render_plot_specs = item_spec.render_plot.clone().unwrap();
    render_plot_specs.read_schema()?;

    let mut templates = Tera::default();
    templates.add_raw_template(
        "plot.html.tera",
        include_str!("../../../templates/plot.html.tera"),
    )?;
    let mut context = Context::new();

    let local: DateTime<Local> = Local::now();

    context.insert("data", &json!(records).to_string());
    context.insert("description", &item_spec.description);
    context.insert(
        "tables",
        &tables
            .iter()
            .filter(|t| !views.get(*t).unwrap().hidden)
            .filter(|t| {
                if let Some(default_view) = default_view {
                    t != &default_view
                } else {
                    true
                }
            })
            .collect_vec(),
    );
    context.insert("default_view", default_view);
    context.insert("name", name);
    context.insert("specs", &render_plot_specs.schema.as_ref().unwrap());
    context.insert("time", &local.format("%a %b %e %T %Y").to_string());
    context.insert("version", &env!("CARGO_PKG_VERSION"));

    let file_path =
        Path::new(output_path.as_ref()).join(Path::new("index_1").with_extension("html"));

    let html = templates.render("plot.html.tera", &context)?;

    let mut file = fs::File::create(file_path)?;
    file.write_all(html.as_bytes())?;

    Ok(())
}

/// Renders a plot page from given render-plot spec containing multiple datasets
fn render_plot_page_with_multiple_datasets<P: AsRef<Path>>(
    output_path: P,
    tables: &[String],
    name: &str,
    item_spec: &ItemSpecs,
    datasets: HashMap<String, &DatasetSpecs>,
    views: &HashMap<String, ItemSpecs>,
    default_view: &Option<String>,
) -> Result<()> {
    let generate_reader = |separator: char, path: &PathBuf| -> csv::Result<Reader<File>> {
        csv::ReaderBuilder::new()
            .delimiter(separator as u8)
            .from_path(&path)
    };

    let mut data = HashMap::new();

    for (name, dataset) in datasets {
        let mut reader = generate_reader(dataset.separator, &dataset.path)
            .context(format!("Could not read file with path {:?}", &dataset.path))?;

        let headers = reader.headers()?.iter().map(|s| s.to_owned()).collect_vec();
        let records: Vec<HashMap<String, String>> = reader
            .records()
            .skip(&dataset.header_rows - 1)
            .map(|row| {
                row.unwrap()
                    .iter()
                    .enumerate()
                    .map(|(index, record)| {
                        (headers.get(index).unwrap().to_owned(), record.to_owned())
                    })
                    .collect()
            })
            .collect_vec();

        data.insert(name.to_string(), records);
    }

    let mut render_plot_specs = item_spec.render_plot.clone().unwrap();
    render_plot_specs.read_schema()?;
    let mut templates = Tera::default();
    templates.add_raw_template(
        "plot.html.tera",
        include_str!("../../../templates/plot.html.tera"),
    )?;
    let mut context = Context::new();

    let local: DateTime<Local> = Local::now();

    context.insert("datasets", &json!(data).to_string());
    context.insert("description", &item_spec.description);
    context.insert(
        "tables",
        &tables
            .iter()
            .filter(|t| !views.get(*t).unwrap().hidden)
            .filter(|t| {
                if let Some(default_view) = default_view {
                    t != &default_view
                } else {
                    true
                }
            })
            .collect_vec(),
    );
    context.insert("default_view", default_view);
    context.insert("name", name);
    context.insert("specs", &render_plot_specs.schema.as_ref().unwrap());
    context.insert("time", &local.format("%a %b %e %T %Y").to_string());
    context.insert("version", &env!("CARGO_PKG_VERSION"));

    let file_path =
        Path::new(output_path.as_ref()).join(Path::new("index_1").with_extension("html"));

    let html = templates.render("plot.html.tera", &context)?;

    let mut file = fs::File::create(file_path)?;
    file.write_all(html.as_bytes())?;

    Ok(())
}

#[derive(Serialize, Debug, Clone, PartialEq)]
struct Linkout {
    name: String,
    url: String,
}

/// Renders the additional column with buttons for tables that contains the linkouts
fn render_link_column(
    row: &[String],
    linked_tables: &LinkedTable,
    titles: &[String],
    links: &HashMap<String, LinkSpec>,
) -> Result<String> {
    let linkouts = render_linkouts(row, linked_tables, titles, links)?;

    let mut templates = Tera::default();
    templates.add_raw_template(
        "linkout_button.html.tera",
        include_str!("../../../templates/linkout_button.html.tera"),
    )?;
    let mut context = Context::new();
    context.insert("links", &linkouts);

    Ok(templates.render("linkout_button.html.tera", &context)?)
}

/// Formats linkouts from given links config
fn render_linkouts(
    row: &[String],
    linked_tables: &LinkedTable,
    titles: &[String],
    links: &HashMap<String, LinkSpec>,
) -> Result<Vec<Linkout>> {
    let mut linkouts = Vec::new();
    for (name, link_specs) in links {
        let index = titles.iter().position(|t| t == &link_specs.column).unwrap();
        if let Some(table) = &link_specs.view {
            let val = table.replace("{value}", &row[index]);
            linkouts.push(Linkout {
                name: name.to_string(),
                url: format!("../{}/index_1.html", val),
            })
        } else if let Some(table_row) = &link_specs.table_row {
            let (table, linked_column) = table_row.split_once('/').unwrap();
            let linked_values = linked_tables
                .get(&(table.to_string(), linked_column.to_string()))
                .unwrap();
            let linked_value = match linked_values.index.get(&row[index]) {
                Some(value) => value,
                None => {
                    if !link_specs.optional {
                        bail!(TableLinkingError::NotFound {
                            not_found: row[index].to_string(),
                            column: linked_column.to_string(),
                            table: table.to_string(),
                        })
                    } else {
                        continue;
                    }
                }
            };
            linkouts.push(Linkout {
                name: name.to_string(),
                url: format!(
                    "../{}/index_{}.html?highlight={}",
                    table,
                    linked_value.page + 1,
                    linked_value.row,
                ),
            });
        } else {
            unreachable!()
        }
    }
    Ok(linkouts)
}

#[derive(Error, Debug)]
pub enum TableLinkingError {
    #[error("Could not find {not_found:?} in column {column:?} of table {table:?}")]
    NotFound {
        not_found: String,
        column: String,
        table: String,
    },
}

#[derive(Error, Debug)]
pub enum ColumnError {
    #[error("Could not find column {column:?} in dataset with path {path:?}. If this is intentional try to use optional: true.")]
    NotFound { column: String, path: String },
}

#[derive(Error, Debug)]
pub enum DatasetError {
    #[error("Could not find dataset {dataset_name:?}.")]
    NotFound { dataset_name: String },
}
