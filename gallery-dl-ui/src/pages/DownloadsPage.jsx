import { useState, useEffect, useRef, useCallback } from 'react';
import { Link } from 'react-router-dom';
import { Download, ExternalLink, RefreshCcw } from 'lucide-react';
import { listRequests, requeueRequest } from '../api';
import StatusBadge from '../components/StatusBadge';
import Pagination from '../components/Pagination';
import './DownloadsPage.css';

export default function DownloadsPage() {
  const [data, setData] = useState(null);
  const [page, setPage] = useState(1);
  const [loading, setLoading] = useState(true);
  const [search, setSearch] = useState('');
  const [debouncedSearch, setDebouncedSearch] = useState('');
  const [sort, setSort] = useState('newest');
  const [status, setStatus] = useState('');
  const pollRef = useRef(null);

  // Debounce search
  useEffect(() => {
    const timer = setTimeout(() => {
      setDebouncedSearch(search);
      setPage(1); // Reset to first page on search
    }, 500);
    return () => clearTimeout(timer);
  }, [search]);

  const fetchData = useCallback(async (p, q, s, st) => {
    try {
      const res = await listRequests(p, 20, q, s, st);
      setData(res);
    } catch (err) {
      console.error('Failed to fetch requests:', err);
    } finally {
      setLoading(false);
    }
  }, []);

  // Initial load and dependencies
  useEffect(() => {
    setLoading(true);
    fetchData(page, debouncedSearch, sort, status);
  }, [page, debouncedSearch, sort, status, fetchData]);

  // Auto-poll when there are active downloads
  useEffect(() => {
    if (!data) return;

    const hasActive = data.data.some((r) =>
      ['pending', 'downloading', 'processing'].includes(r.status)
    );

    if (hasActive) {
      pollRef.current = setInterval(() => fetchData(page, debouncedSearch, sort, status), 3000);
    }

    return () => {
      if (pollRef.current) clearInterval(pollRef.current);
    };
  }, [data, page, debouncedSearch, sort, status, fetchData]);

  const isPolling = data?.data.some((r) =>
    ['pending', 'downloading', 'processing'].includes(r.status)
  );

  const handleRequeue = async (id) => {
    try {
      await requeueRequest(id);
      fetchData(page);
    } catch (err) {
      alert(`Failed to requeue: ${err.message}`);
    }
  };

  if (loading && !data) {
    return <div className="empty-state"><p>Loading...</p></div>;
  }

  return (
    <div>
      <div className="page-header">
        <h2>
          Downloads
          {isPolling && (
            <span className="polling-indicator">
              <span className="polling-dot" />
              Auto-refreshing
            </span>
          )}
        </h2>
        <p>Track all download requests and their status</p>
      </div>

      <div className="downloads-toolbar">
        <div className="search-box">
          <input
            type="text"
            placeholder="Search by URL or title..."
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            className="search-input"
          />
        </div>
        <div className="filter-box">
          <select
            value={status}
            onChange={(e) => {
              setStatus(e.target.value);
              setPage(1);
            }}
            className="filter-select"
          >
            <option value="">All Statuses</option>
            <option value="pending">Pending</option>
            <option value="processing">Processing</option>
            <option value="completed">Completed</option>
            <option value="failed">Failed</option>
          </select>
        </div>
        <div className="sort-box">
          <select 
            value={sort} 
            onChange={(e) => {
              setSort(e.target.value);
              setPage(1);
            }}
            className="sort-select"
          >
            <option value="newest">Newest First</option>
            <option value="oldest">Oldest First</option>
            <option value="status_asc">Status (A-Z)</option>
            <option value="status_desc">Status (Z-A)</option>
            <option value="title_asc">Title (A-Z)</option>
            <option value="title_desc">Title (Z-A)</option>
            <option value="url_asc">URL (A-Z)</option>
            <option value="url_desc">URL (Z-A)</option>
          </select>
        </div>
      </div>

      {data?.data.length === 0 ? (
        <div className="empty-state">
          <Download size={48} />
          <h3>No downloads yet</h3>
          <p>Submit a URL in the bar above to get started</p>
        </div>
      ) : (
        <>
          <div className="downloads-list">
            {data?.data.map((req) => (
              <div key={req.id} className="download-item">
                <div className="download-url">
                  <Link to={`/downloads/${req.id}`} title={req.url}>
                    {req.title || req.url}
                  </Link>
                  <div className="download-meta">
                    {req.title && <span className="meta-url">{req.url}</span>}
                    <span>{new Date(req.created_at + 'Z').toLocaleString()}</span>
                    <span>ID: {req.id.slice(0, 8)}…</span>
                  </div>
                  {req.error_message && (
                    <div className="download-error" title={req.error_message}>
                      {req.error_message}
                    </div>
                  )}
                </div>
                <div className="download-actions">
                  <StatusBadge status={req.status} />
                  <a
                    href={req.url}
                    target="_blank"
                    rel="noopener noreferrer"
                    className="btn btn-ghost"
                    title="Open source URL"
                  >
                    <ExternalLink size={14} />
                  </a>
                  <button
                    onClick={() => handleRequeue(req.id)}
                    className="btn btn-ghost"
                    title="Re-queue (purge and restart)"
                    disabled={['pending', 'downloading', 'processing'].includes(req.status)}
                  >
                    <RefreshCcw size={14} />
                  </button>
                </div>
              </div>
            ))}
          </div>
          {data?.pagination && (
            <Pagination
              page={data.pagination.page}
              totalPages={data.pagination.total_pages}
              total={data.pagination.total}
              onPageChange={setPage}
            />
          )}
        </>
      )}
    </div>
  );
}
