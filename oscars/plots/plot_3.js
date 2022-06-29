let show_plot_3 = true;
let plot_3 = {
    "$schema": "https://vega.github.io/schema/vega-lite/v4.json",
    "width": "container",
    "layer": [
        {
            "data": {"values": [{"key":"Katharine Hepburn","value":4},{"key":"Daniel Day-Lewis","value":3},{"key":"Luise Rainer","value":2},{"key":"Dustin Hoffman","value":2},{"key":"Jack Nicholson","value":2},{"key":"Meryl Streep","value":2},{"key":"Olivia de Havilland","value":2},{"key":"Jodie Foster","value":2},{"key":"Hilary Swank","value":2},{"key":"Spencer Tracy","value":2}]},
            "mark": "bar",
            "encoding": {
                "x": {
                    "field": "key",
                    "sort": {"field": "value", "order": "descending"},
                    "title": "name"
                },
                "y": {"field": "value", "type": "quantitative", "title": null}
            }
        }
    ]
};
