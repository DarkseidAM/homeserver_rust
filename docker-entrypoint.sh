#!/bin/bash
set -e

# Default values if not set
PUID=${PUID:-1000}
PGID=${PGID:-${PUID}}

# Get Docker group GID from host if available, otherwise use PGID
if [ -e /var/run/docker.sock ]; then
    # Try to get docker group GID from mounted socket's group
    DOCKER_GID=$(stat -c '%g' /var/run/docker.sock 2>/dev/null || echo "${PGID}")
else
    DOCKER_GID=${PGID}
fi

# Create group if it doesn't exist
if ! getent group "${PGID}" > /dev/null 2>&1; then
    groupadd -g "${PGID}" serveruser
fi

# Create user if it doesn't exist
if ! id -u serveruser > /dev/null 2>&1; then
    useradd -r -u "${PUID}" -g "${PGID}" -m -s /sbin/nologin serveruser
else
    # User exists, ensure it has the right UID/GID
    usermod -u "${PUID}" -g "${PGID}" serveruser 2>/dev/null || true
fi

# Add user to docker group if docker socket exists
if [ -e /var/run/docker.sock ]; then
    # Get docker group GID from socket
    DOCKER_GID=$(stat -c '%g' /var/run/docker.sock 2>/dev/null || echo "${DOCKER_GID}")
    
    # Find the group that matches the Docker socket GID
    DOCKER_GROUP=$(getent group "${DOCKER_GID}" 2>/dev/null | cut -d: -f1 || echo "")
    
    if [ -n "${DOCKER_GROUP}" ]; then
        # Group with matching GID exists, use it
        usermod -aG "${DOCKER_GROUP}" serveruser 2>/dev/null || true
    else
        # No group with this GID exists, try to create docker group
        # First check if docker group exists by name
        if ! getent group docker > /dev/null 2>&1; then
            # Create docker group with the detected GID
            groupadd -g "${DOCKER_GID}" docker 2>/dev/null || groupadd docker 2>/dev/null || true
        fi
        # Add user to docker group
        usermod -aG docker serveruser 2>/dev/null || true
    fi
fi

# Fix ownership of app directory
chown -R serveruser:serveruser /app

# Switch to serveruser and execute
# Use su to switch user, then exec the command with tini
# The 'sh' argument becomes $0, and "$@" are the actual command arguments
exec /usr/bin/tini -- su serveruser -s /bin/sh -c 'exec "$@"' sh "$@"
