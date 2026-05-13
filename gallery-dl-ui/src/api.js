const API_BASE = 'http://localhost:3000';

/**
 * Generic fetch wrapper with JSON parsing and error handling.
 */
async function request(path, options = {}) {
  const url = `${API_BASE}${path}`;
  const res = await fetch(url, {
    headers: { 'Content-Type': 'application/json' },
    ...options,
  });
  if (!res.ok) {
    const body = await res.json().catch(() => ({}));
    throw new Error(body.error || `Request failed: ${res.status}`);
  }
  return res.json();
}

// --- Requests ---
 
export function createRequest(url, name = null) {
  return request('/api/requests', {
    method: 'POST',
    body: JSON.stringify({ url, name }),
  });
}

export function listRequests(page = 1, perPage = 50, q = '', sort = '', status = '') {
  let url = `/api/requests?page=${page}&per_page=${perPage}`;
  if (q) url += `&q=${encodeURIComponent(q)}`;
  if (sort) url += `&sort=${sort}`;
  if (status) url += `&status=${status}`;
  return request(url);
}

export function getRequest(id) {
  return request(`/api/requests/${id}`);
}

export function requeueRequest(id) {
  return request(`/api/requests/${id}/requeue`, {
    method: 'POST',
  });
}

// --- Galleries ---

export function listGalleries(page = 1, perPage = 50) {
  return request(`/api/galleries?page=${page}&per_page=${perPage}`);
}

export function getGallery(id) {
  return request(`/api/galleries/${id}`);
}

export function updateGallery(id, title) {
  return request(`/api/galleries/${id}`, {
    method: 'PATCH',
    body: JSON.stringify({ title }),
  });
}

// --- Images ---

export function listImages(page = 1, perPage = 50) {
  return request(`/api/images?page=${page}&per_page=${perPage}`);
}

// --- Videos ---

export function listVideos(page = 1, perPage = 50) {
  return request(`/api/videos?page=${page}&per_page=${perPage}`);
}

export function getVideoProgress(id) {
  return request(`/api/videos/${id}/progress`);
}

export function saveVideoProgress(id, positionSeconds) {
  return request(`/api/videos/${id}/progress`, {
    method: 'POST',
    body: JSON.stringify({ position_seconds: positionSeconds }),
  });
}

// --- Media URLs ---

export function imageUrl(hash, extension) {
  return `${API_BASE}/media/images/${hash}.${extension}`;
}

export function thumbnailUrl(hash) {
  return `${API_BASE}/media/thumbnails/${hash}.jpg`;
}

export function videoUrl(hash, extension) {
  return `${API_BASE}/media/videos/${hash}.${extension}`;
}

export function trickplayUrl(hash) {
  return `${API_BASE}/media/trickplay/${hash}.jpg`;
}
