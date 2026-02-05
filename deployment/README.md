# Homeserver Deployment

This folder contains production-ready deployment files for running Homeserver using pre-built Docker images from GitHub Container Registry (GHCR).

## Quick Start

1. **Copy the example environment file:**
   ```bash
   cp .env.example .env
   ```

2. **Edit `.env` to customize:**
   ```bash
   # Set your user/group IDs (get with: id -u and id -g)
   PUID=1000
   PGID=1000
   
   # Set your timezone
   TZ=America/New_York
   
   # Set log level (warn for production, info for debugging)
   RUST_LOG=warn
   ```

3. **Customize `config.toml` if needed:**
   - Change `retention_days` to keep more/less history
   - Adjust `sample_interval_ms` for different monitoring frequency
   - Modify `port` if 8081 conflicts with other services

4. **Update the image name in `docker-compose.yml`:**
   ```yaml
   image: ghcr.io/OWNER/REPO:latest
   ```
   Replace `OWNER/REPO` with your actual GitHub repository path (e.g., `username/homeserver-rust`).

5. **Start the service:**
   ```bash
   docker compose up -d
   ```

6. **Check logs:**
   ```bash
   docker compose logs -f
   ```

7. **Access the API:**
   ```bash
   curl http://localhost:8081/api/info
   ```

## Directory Structure

```
deployment/
├── docker-compose.yml  # Production compose file (pulls from GHCR)
├── config.toml         # Application configuration
├── .env.example        # Example environment variables
├── .env                # Your actual environment (created by you, not in git)
├── data/               # SQLite database and persistent data (auto-created)
└── README.md           # This file
```

## Configuration Files

### `docker-compose.yml`
- Pulls pre-built image from GHCR
- Mounts Docker socket for container monitoring
- Configures resource limits, logging, and health checks
- Uses environment variables from `.env`

### `config.toml`
- Application-level configuration
- Controls monitoring frequency, database settings, etc.
- Mounted as read-only into the container

### `.env`
- Environment-specific settings (user IDs, timezone, log level)
- Not committed to git (use `.env.example` as template)

## Updating

Pull the latest image and restart:
```bash
docker compose pull
docker compose up -d
```

## Monitoring

- **WebSocket Endpoints:**
  - `ws://localhost:8081/ws/cpu` - Real-time CPU stats
  - `ws://localhost:8081/ws/ram` - Real-time RAM stats
  - `ws://localhost:8081/ws/system` - Full system snapshots

- **REST Endpoints:**
  - `GET /api/info` - Static system information
  - `GET /api/history/recent?limit=100` - Recent snapshots

## Troubleshooting

### Permission Issues
If you see "permission denied" errors for Docker socket:
- Ensure `PUID` and `PGID` in `.env` match your host user
- The entrypoint automatically handles Docker socket permissions

### Database Issues
- Database is stored in `./data/server.db`
- To reset: `docker compose down && rm -rf data && docker compose up -d`

### High Memory Usage
- Adjust `memory: 256M` limit in `docker-compose.yml` if needed
- Check `retention_days` and `broadcast_capacity` in `config.toml`

### Logs Not Rotating
- Log rotation is configured in `docker-compose.yml`
- Max 3 files × 10MB = 30MB total
- Logs are compressed automatically

## Advanced Configuration

### Custom Port
Change in `docker-compose.yml`:
```yaml
ports:
  - "9090:8081"  # Host:Container
```

### Multiple Instances
Run multiple instances by:
1. Copy deployment folder (e.g., `deployment-server2`)
2. Change `container_name` and `ports` in `docker-compose.yml`
3. Use different data directories

### Production Hardening
- Set `RUST_LOG=warn` to minimize logs
- Enable firewall rules for port 8081
- Use reverse proxy (nginx/traefik) for HTTPS
- Set up monitoring/alerts for container health

## Support

For issues, feature requests, or questions:
- GitHub Issues: https://github.com/YOUR_USERNAME/homeserver-rust/issues
- Documentation: https://github.com/YOUR_USERNAME/homeserver-rust

## License

MIT
