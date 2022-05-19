let show_plot_0 = true;
let plot_0 = {
    "$schema": "https://vega.github.io/schema/vega-lite/v4.json",
    "width": "container",
    "layer": [
        {
            "data": {"values": [{"key":"Coming Home","value":2},{"key":"It Happened One Night","value":2},{"key":"On Golden Pond","value":2},{"key":"The Silence of the Lambs","value":2},{"key":"Network","value":2},{"key":"One Flew Over the Cuckoo's Nest","value":1},{"key":"Boys Town","value":1},{"key":"Erin Brockovich","value":1},{"key":"Monster","value":1},{"key":"Wall Street","value":1}]},
            "mark": "bar",
            "encoding": {
                "x": {
                    "field": "key",
                    "sort": {"field": "value", "order": "descending"},
                    "title": "Title"
                },
                "y": {"field": "value", "type": "quantitative", "title": null}
            }
        }
    ]
};
