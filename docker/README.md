# Apache Prometheus Exporter - Docker Example

Here you can find an example Docker Compose configuration you can use to develop and test the exporter.

This configuration will create a Docker volume for the logs, and the following containers:

1. **Apache** running on 3 ports, each of which has its own access log:
   - http://localhost:2001
   - http://localhost:2002
   - http://localhost:2003
2. **Grafana** running on http://localhost:2000 with a pre-configured Prometheus data source.
   - **User** : `admin`
   - **Password** : `admin`
3. **Prometheus** configured with the exporter's endpoint.
4. **Exporter** built using the source code from this repository.

This example is not suitable for production. You can use it as inspiration, but you will have to modify it in order to persist container data and follow the latest security practices:

- Create Docker volumes for persistent storage of container data and configuration files
- Create a dedicated user for each container instead of running as `root`
- Customize the configuration of every containerized application for your needs
- Use HTTPS for all domains served by Apache
- Have Apache act as a reverse proxy for Grafana instead of exposing Grafana's web server port
- Use a strong password for Grafana and pass it via Docker secrets instead of environment variables
