let show_plot_8 = true;
let plot_8 = {
    "$schema": "https://vega.github.io/schema/vega-lite/v4.json",
    "width": "container",
    "layer": [
        {
            "data": {"values": [{"key":"tt0102926","value":2},{"key":"tt0119822","value":2},{"key":"tt0025316","value":2},{"key":"tt0073486","value":2},{"key":"tt0074958","value":2},{"key":"tt0082846","value":2},{"key":"tt0077362","value":2},{"key":"tt0048356","value":1},{"key":"tt0039335","value":1},{"key":"tt0107818","value":1}]},
            "mark": "bar",
            "encoding": {
                "x": {
                    "field": "key",
                    "sort": {"field": "value", "order": "descending"},
                    "title": "imdbID"
                },
                "y": {"field": "value", "type": "quantitative", "title": null}
            }
        }
    ]
};