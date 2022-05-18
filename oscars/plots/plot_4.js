let show_plot_4 = true;
let plot_4 = {
    "$schema": "https://vega.github.io/schema/vega-lite/v4.json",
    "width": "container",
    "layer": [
        {
            "data": {"values": [{"key":"Network","value":2},{"key":"On Golden Pond","value":2},{"key":"The Silence of the Lambs","value":2},{"key":"Coming Home","value":2},{"key":"It Happened One Night","value":2},{"key":"The Miracle Worker","value":1},{"key":"Room at the Top","value":1},{"key":"Mary Poppins","value":1},{"key":"Boys Don't Cry","value":1},{"key":"Johnny Belinda","value":1}]},
            "mark": "bar",
            "encoding": {
                "x": {
                    "field": "key",
                    "sort": {"field": "value", "order": "descending"},
                    "title": "movie"
                },
                "y": {"field": "value", "type": "quantitative", "title": null}
            }
        }
    ]
};
