let show_plot_3 = true;
let plot_3 = {
    "$schema": "https://vega.github.io/schema/vega-lite/v4.json",
    "width": "container",
    "layer": [
        {
            "data": {"values": [{"key":"22 Feb 1934","value":2},{"key":"19 Nov 1975","value":2},{"key":"30 Jan 2009","value":2},{"key":"14 Feb 1991","value":2},{"key":"04 Mar 1983","value":2},{"key":"15 Feb 1978","value":2},{"key":"12 Feb 1982","value":2},{"key":"25 Dec 1997","value":2},{"key":"27 Nov 1976","value":2},{"key":"12 Mar 1946","value":1}]},
            "mark": "bar",
            "encoding": {
                "x": {
                    "field": "key",
                    "sort": {"field": "value", "order": "descending"},
                    "title": "Released"
                },
                "y": {"field": "value", "type": "quantitative", "title": null}
            }
        }
    ]
};
