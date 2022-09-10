# Apache Prometheus Exporter

Exports Prometheus metrics from Apache access logs.

See the [docker](./docker) folder for an example setup using Docker Compose.

## 1. Configure Apache Access Log Format

The following snippet will create a log format named `prometheus` that includes all information the exporter expects. See [Apache documentation](https://httpd.apache.org/docs/2.4/mod/mod_log_config.html#formats) for explanation of the format.

```apache
LogFormat "%t %h \"%r\" %>s %O %{ms}T \"%{Referer}i\" \"%{User-Agent}i\"" prometheus
```

## 2. Configure Apache Virtual Hosts

The following snippet is an example of how you could configure Apache to serve 3 domains from different folders using macros.

Each domain has its own access and error log file. The log files are rotated daily, with a dedicated folder for each day, and a `${APACHE_LOG_DIR}/latest/` folder with hard links to today's log files - this folder will be watched by the exporter.

```apache
<Macro Logs $domain>
	ErrorLog "|/usr/bin/rotatelogs -l -f -D -L ${APACHE_LOG_DIR}/latest/$domain.error.log ${APACHE_LOG_DIR}/%Y-%m-%d/$domain.error.log 86400"
	CustomLog "|/usr/bin/rotatelogs -l -f -D -L ${APACHE_LOG_DIR}/latest/$domain.access.log ${APACHE_LOG_DIR}/%Y-%m-%d/$domain.access.log 86400" prometheus
</Macro>

<Macro Domain $domain>
	<VirtualHost *:80>
		ServerName $domain
		DocumentRoot /var/www/html/$domain
		Use Logs $domain
	</VirtualHost>
</Macro>

Domain first.example.com
Domain second.example.com
Domain third.example.com

UndefMacro Domain
UndefMacro Logs
```

In this example, the `first.example.com` domain will be served from `/var/www/html/first.example.com`, and its logs will be written to:
- `${APACHE_LOG_DIR}/latest/first.example.com.access.log`
- `${APACHE_LOG_DIR}/latest/first.example.com.error.log`

## 3. Configure the Exporter

The exporter requires the following environment variables:

### `HTTP_HOST`

The host that the HTTP server for metrics will listen on. If omitted, defaults to `127.0.0.1`.

### `ACCESS_LOG_FILE_PATTERN`, `ERROR_LOG_FILE_PATTERN`

The path to the access/error log files. You may use a single wildcard to match multiple files in a folder, or to match multiple folders in one level of the path. Whatever is matched by the wildcard will become the Prometheus label `file`. If there is no wildcard, the `file` label will be empty.

#### Example 1 (File Name Wildcard)

Log files for all domains are in `/var/log/apache2/latest/` and are named `<domain>.access.log` and `<domain>.error.log`. This is the set up from the Apache configuration example above.

**Pattern:** `/var/log/apache2/latest/*.access.log`

- Metrics for `/var/log/apache2/latest/first.example.com.access.log` will be labeled: `first.example.com`
- Metrics for `/var/log/apache2/latest/first.example.com.error.log` will be labeled: `first.example.com`
- Metrics for `/var/log/apache2/latest/second.example.com.access.log` will be labeled: `second.example.com`

The wildcard may appear anywhere in the file name.

#### Example 2 (Folder Wildcard)

Every domain has its own folder in `/var/log/apache2/latest/` containing log files named `access.log` and `error.log`.

**Pattern:** `/var/log/apache2/latest/*/access.log`

- Metrics for `/var/log/apache2/latest/first.example.com/access.log` will be labeled: `first.example.com`
- Metrics for `/var/log/apache2/latest/first.example.com/error.log` will be labeled: `first.example.com`
- Metrics for `/var/log/apache2/latest/second.example.com/access.log` will be labeled: `second.example.com`

The wildcard must not include any prefix or suffix, so `/*/` is accepted, but `/prefix_*/` or `/*_suffix/` is not.

#### Notes

> At least one access log file and one error log file must be found when the exporter starts, otherwise the exporter immediately exits with an error.

> If a log file is deleted, the exporter will automatically resume watching it if it is re-created later. If you want the exporter to forget about deleted log files, restart the exporter.

## 4. Launch the Exporter

Start the exporter. The standard output will show which log files have been found, the web server host, and the metrics endpoint URL.

Press `Ctrl-C` to stop the exporter.

## 5. Collect Prometheus Metrics

Currently, the exporter exposes only these metrics:

- `apache_requests_total` total number of requests
- `apache_errors_total` total number of errors

More detailed metrics will be added in the future.
