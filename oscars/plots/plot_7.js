let show_plot_7 = true;
let plot_7 = {
    "$schema": "https://vega.github.io/schema/vega-lite/v4.json",
    "width": "container",
    "layer": [
        {
            "data": {"values": [{"key":"1907-05-12","value":4},{"key":"1957-04-29","value":3},{"key":"1932-02-27","value":2},{"key":"1915-08-29","value":2},{"key":"1910-01-12","value":2},{"key":"1957-06-23","value":2},{"key":"1916-07-01","value":2},{"key":"1901-05-07","value":2},{"key":"1960-08-17","value":2},{"key":"1897-08-31","value":2}]},
            "mark": "bar",
            "encoding": {
                "x": {
                    "field": "key",
                    "sort": {"field": "value", "order": "descending"},
                    "title": "birth date"
                },
                "y": {"field": "value", "type": "quantitative", "title": null}
            }
        }
    ]
};