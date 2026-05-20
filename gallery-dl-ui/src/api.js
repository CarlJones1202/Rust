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
  if (res.status === 204) return null;
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

export function guessRequestTitle(url) {
  return request(`/api/requests/guess-title?url=${encodeURIComponent(url)}`);
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

export function retroactiveUpdateTitles() {
  return request('/api/galleries/retroactive-update', {
    method: 'POST',
  });
}

// --- Images ---

export function listImages(page = 1, perPage = 50, favorites = false) {
  let url = `/api/images?page=${page}&per_page=${perPage}`;
  if (favorites) url += '&favorites=true';
  return request(url);
}

export function toggleFavorite(id, isFavorite) {
  return request(`/api/images/${id}/favorite`, {
    method: 'PATCH',
    body: JSON.stringify({ is_favorite: isFavorite }),
  });
}

// --- Videos ---
export function listVideos(page = 1, perPage = 50) {
  return request(`/api/videos?page=${page}&per_page=${perPage}`);
}

export function updateVideo(id, title) {
  return request(`/api/videos/${id}`, {
    method: 'PATCH',
    body: JSON.stringify({ title }),
  });
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

// --- People ---

export function listPersons(page = 1, perPage = 50, q = '') {
  let url = `/api/persons?page=${page}&per_page=${perPage}`;
  if (q) url += `&q=${encodeURIComponent(q)}`;
  return request(url);
}

export function createPerson(name, aliases = []) {
  return request('/api/persons', {
    method: 'POST',
    body: JSON.stringify({ name, aliases }),
  });
}

export function getPerson(id) {
  return request(`/api/persons/${id}`);
}

export function updatePerson(id, data) {
  return request(`/api/persons/${id}`, {
    method: 'PATCH',
    body: JSON.stringify(data),
  });
}

export function deletePerson(id) {
  return request(`/api/persons/${id}`, {
    method: 'DELETE',
  });
}

export function uploadPersonImage(id, file) {
  const formData = new FormData();
  formData.append('image', file);
  
  return fetch(`${API_BASE}/api/persons/${id}/images`, {
    method: 'POST',
    body: formData,
  }).then(res => {
    if (!res.ok) throw new Error('Upload failed');
    return res.json();
  });
}

export function deletePersonImage(personId, imageId) {
  return request(`/api/persons/${personId}/images/${imageId}`, {
    method: 'DELETE',
  });
}

export function setPersonPrimaryImage(personId, imageId) {
  return request(`/api/persons/${personId}/images/${imageId}/primary`, {
    method: 'PATCH',
  });
}

export function linkGalleryPerson(personId, galleryId) {
  return request(`/api/persons/${personId}/galleries/${galleryId}`, {
    method: 'POST',
  });
}

export function unlinkGalleryPerson(personId, galleryId) {
  return request(`/api/persons/${personId}/galleries/${galleryId}`, {
    method: 'DELETE',
  });
}

export function listPersonGalleries(personId) {
  return request(`/api/persons/${personId}/galleries`);
}

export function relinkPerson(personId) {
  return request(`/api/persons/${personId}/relink`, {
    method: 'POST',
  });
}

// --- StashDB ---

export function searchStashDB(q) {
  return request(`/api/stashdb/search?q=${encodeURIComponent(q)}`);
}

export function importFromStashDB(personId, stashdbId) {
  return request(`/api/persons/${personId}/stashdb-import`, {
    method: 'POST',
    body: JSON.stringify({ stashdb_id: stashdbId }),
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

export function personImageUrl(hash, extension) {
  return `${API_BASE}/media/persons/${hash}.${extension}`;
}
