let show_plot_6 = true;
let plot_6 = {
    "$schema": "https://vega.github.io/schema/vega-lite/v4.json",
    "width": "container",
    "layer": [
        {
            "data": {"values": [{"key":"William Wyler","value":7},{"key":"George Cukor","value":5},{"key":"Martin Scorsese","value":3},{"key":"Milos Forman","value":3},{"key":"Jonathan Demme","value":3},{"key":"Daniel Mann","value":3},{"key":"James L. Brooks","value":3},{"key":"Delbert Mann","value":2},{"key":"Hal Ashby","value":2},{"key":"Robert Z. Leonard","value":2}]},
            "mark": "bar",
            "encoding": {
                "x": {
                    "field": "key",
                    "sort": {"field": "value", "order": "descending"},
                    "title": "Director"
                },
                "y": {"field": "value", "type": "quantitative", "title": null}
            }
        }
    ]
};