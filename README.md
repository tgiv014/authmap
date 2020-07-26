# authmap
Glue layer for visualizing ssh connection attempts in Grafana.

This service waits for new lines in /var/log/auth.log, parses them, pulls their location using the geoip database, and spits the results into influxdb.

# Usage
- `git clone https://github.com/tgiv014/authmap`
- Download the maxmind geolite2 geoip database and place GeoLite2-City.mmdb in the project root
- `cargo run`
