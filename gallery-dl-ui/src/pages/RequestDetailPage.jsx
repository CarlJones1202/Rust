import { useState, useEffect, useCallback, useRef } from 'react';
import { useParams, Link, useNavigate } from 'react-router-dom';
import { ArrowLeft, ExternalLink, RefreshCcw, Trash2, Pencil, Check, X, Image, Film, AlertCircle, Link as LinkIcon, Plus } from 'lucide-react';
import { getRequest, updateRequest, requeueRequest, deleteRequest, guessRequestTitle } from '../api';
import StatusBadge from '../components/StatusBadge';
import MediaGrid from '../components/MediaGrid';
import Pagination from '../components/Pagination';
import './RequestDetailPage.css';

export default function RequestDetailPage() {
  const { id } = useParams();
  const navigate = useNavigate();
  const [data, setData] = useState(null);
  const [loading, setLoading] = useState(true);
  const [isEditingName, setIsEditingName] = useState(false);
  const [nameDraft, setNameDraft] = useState('');
  const [isEditingBackup, setIsEditingBackup] = useState(false);
  const [backupDraft, setBackupDraft] = useState('');
  const [activeTab, setActiveTab] = useState('galleries');
  const [galleryPage, setGalleryPage] = useState(1);
  const [galleryDetails, setGalleryDetails] = useState({});
  const pollRef = useRef(null);

  const fetchData = useCallback(async () => {
    try {
      const res = await getRequest(id);
      setData(res);
    } catch (err) {
      console.error('Failed to load request:', err);
    } finally {
      setLoading(false);
    }
  }, [id]);

  useEffect(() => {
    fetchData();
  }, [fetchData]);

  useEffect(() => {
    if (!data) return;
    const active = ['pending', 'downloading', 'processing'].includes(data.status);
    if (active) {
      pollRef.current = setInterval(fetchData, 3000);
    }
    return () => {
      if (pollRef.current) clearInterval(pollRef.current);
    };
  }, [data, fetchData]);

  useEffect(() => {
    if (!data?.galleries) return;
    setGalleryDetails((prev) => {
      const next = { ...prev };
      data.galleries.forEach((g) => {
        if (!next[g.id]) next[g.id] = { loading: true };
      });
      return next;
    });
  }, [data]);

  const startEditingName = () => {
    setNameDraft(data?.title || '');
    setIsEditingName(true);
  };

  const saveName = async () => {
    try {
      await updateRequest(id, { name: nameDraft.trim() || null });
      setIsEditingName(false);
      fetchData();
    } catch (err) {
      alert(`Failed to update name: ${err.message}`);
    }
  };

  const startEditingBackup = () => {
    setBackupDraft(data?.backup_url || '');
    setIsEditingBackup(true);
  };

  const saveBackup = async () => {
    try {
      await updateRequest(id, { backup_url: backupDraft.trim() || null });
      setIsEditingBackup(false);
      fetchData();
    } catch (err) {
      alert(`Failed to update backup URL: ${err.message}`);
    }
  };

  const handleRequeue = async () => {
    if (!window.confirm('Re-queue this request? All downloaded media will be purged and the download will restart.')) return;
    try {
      await requeueRequest(id);
      fetchData();
    } catch (err) {
      alert(`Failed to requeue: ${err.message}`);
    }
  };

  const handleDelete = async () => {
    if (!window.confirm('Delete this request and all of its media? This cannot be undone.')) return;
    try {
      await deleteRequest(id);
      navigate('/');
    } catch (err) {
      alert(`Failed to delete: ${err.message}`);
    }
  };

  const handleGuessTitle = async () => {
    try {
      const res = await guessRequestTitle(data.url);
      if (res.title) {
        await updateRequest(id, { name: res.title });
        fetchData();
      }
    } catch (err) {
      alert(`Failed to guess title: ${err.message}`);
    }
  };

  if (loading && !data) {
    return <div className="empty-state"><p>Loading...</p></div>;
  }

  if (!data) {
    return (
      <div className="empty-state">
        <h3>Request not found</h3>
        <Link to="/" className="btn btn-ghost" style={{ marginTop: '12px' }}>
          <ArrowLeft size={14} /> Back to Downloads
        </Link>
      </div>
    );
  }

  const { request, galleries, videos } = data;
  const galleriesTotalPages = Math.max(1, Math.ceil(galleries.length / 12));
  const paginatedGalleries = galleries.slice((galleryPage - 1) * 12, galleryPage * 12);
  const isActive = ['pending', 'downloading', 'processing'].includes(request.status);

  return (
    <div className="request-detail">
      <div className="detail-nav">
        <Link to="/" className="back-link">
          <ArrowLeft size={16} /> Back to Downloads
        </Link>
        <div className="nav-actions">
          <a
            href={request.url}
            target="_blank"
            rel="noopener noreferrer"
            className="btn btn-ghost"
            title="Open source URL"
          >
            <ExternalLink size={14} /> Source
          </a>
          <button
            className="btn btn-ghost"
            onClick={handleRequeue}
            disabled={isActive}
            title="Purge and restart"
          >
            <RefreshCcw size={14} /> Re-queue
          </button>
          <button
            className="btn btn-ghost"
            onClick={handleDelete}
            disabled={isActive}
            title="Delete request and all media"
          >
            <Trash2 size={14} />
          </button>
        </div>
      </div>

      <div className="request-header">
        <div className="header-main">
          <div className="title-section">
            {isEditingName ? (
              <div className="edit-row">
                <input
                  type="text"
                  value={nameDraft}
                  onChange={(e) => setNameDraft(e.target.value)}
                  className="title-input-large"
                  autoFocus
                  onKeyDown={(e) => {
                    if (e.key === 'Enter') saveName();
                    if (e.key === 'Escape') setIsEditingName(false);
                  }}
                />
                <button className="btn btn-primary" onClick={saveName}><Check size={16} /></button>
                <button className="btn btn-ghost" onClick={() => setIsEditingName(false)}><X size={16} /></button>
              </div>
            ) : (
              <div className="display-row">
                <h2>{request.title || request.url}</h2>
                <button className="btn-icon btn-icon-sm" onClick={startEditingName} title="Edit name"><Pencil size={14} /></button>
                {!request.title && (
                  <button className="btn btn-ghost" onClick={handleGuessTitle} title="Guess title from URL">
                    Guess Title
                  </button>
                )}
              </div>
            )}
            {request.title && (
              <a
                href={request.url}
                target="_blank"
                rel="noopener noreferrer"
                className="request-url-link"
              >
                {request.url}
              </a>
            )}
          </div>
        </div>

        <div className="header-meta">
          <div className="meta-row">
            <span className="meta-label">Status</span>
            <StatusBadge status={request.status} />
            {isActive && <span className="polling-indicator"><span className="polling-dot" /> Auto-refreshing</span>}
          </div>
          <div className="meta-row">
            <span className="meta-label">Created</span>
            <span>{new Date(request.created_at + 'Z').toLocaleString()}</span>
          </div>
          <div className="meta-row">
            <span className="meta-label">Updated</span>
            <span>{new Date(request.updated_at + 'Z').toLocaleString()}</span>
          </div>
          <div className="meta-row">
            <span className="meta-label">Stats</span>
            <span>{request.image_count} image(s), {request.video_count} video(s)</span>
          </div>
        </div>
      </div>

      {request.error_message && (
        <div className="error-banner">
          <AlertCircle size={16} />
          <span>{request.error_message}</span>
        </div>
      )}

      <section className="backup-section">
        <div className="section-header">
          <h3>
            <LinkIcon size={16} /> Backup URL
          </h3>
        </div>
        {isEditingBackup ? (
          <div className="backup-edit-row">
            <input
              type="text"
              value={backupDraft}
              onChange={(e) => setBackupDraft(e.target.value)}
              placeholder="https://backup-url.example.com/..."
              className="backup-input"
              autoFocus
              onKeyDown={(e) => {
                if (e.key === 'Enter') saveBackup();
                if (e.key === 'Escape') setIsEditingBackup(false);
              }}
            />
            <button className="btn btn-primary btn-sm" onClick={saveBackup}><Check size={14} /></button>
            <button className="btn btn-ghost btn-sm" onClick={() => setIsEditingBackup(false)}><X size={14} /></button>
          </div>
        ) : request.backup_url ? (
          <div className="backup-display">
            <a href={request.backup_url} target="_blank" rel="noopener noreferrer">{request.backup_url}</a>
            <button className="btn-icon btn-icon-sm" onClick={startEditingBackup} title="Edit backup URL"><Pencil size={12} /></button>
          </div>
        ) : (
          <button className="btn btn-ghost btn-sm" onClick={startEditingBackup}>
            <Plus size={14} /> Add backup URL
          </button>
        )}
      </section>

      <div className="tabs">
        <button
          className={`tab ${activeTab === 'galleries' ? 'active' : ''}`}
          onClick={() => setActiveTab('galleries')}
        >
          <Image size={14} /> Galleries ({galleries.length})
        </button>
        <button
          className={`tab ${activeTab === 'videos' ? 'active' : ''}`}
          onClick={() => setActiveTab('videos')}
        >
          <Film size={14} /> Videos ({videos.length})
        </button>
      </div>

      {activeTab === 'galleries' && (
        <section className="tab-content">
          {galleries.length === 0 ? (
            <div className="empty-substate">No galleries created from this request yet.</div>
          ) : (
            <>
              <MediaGrid
                items={paginatedGalleries}
                onItemClick={(g) => navigate(`/galleries/${g.id}`)}
                renderItem={(g) => (
                  <div className="gallery-thumb-card">
                    <div className="gallery-thumb-placeholder">
                      <Image size={20} />
                    </div>
                    <div className="gallery-thumb-name">{g.title || g.id.slice(0, 8)}</div>
                  </div>
                )}
              />
              {galleriesTotalPages > 1 && (
                <Pagination
                  page={galleryPage}
                  totalPages={galleriesTotalPages}
                  total={galleries.length}
                  onPageChange={setGalleryPage}
                />
              )}
            </>
          )}
        </section>
      )}

      {activeTab === 'videos' && (
        <section className="tab-content">
          {videos.length === 0 ? (
            <div className="empty-substate">No videos downloaded from this request yet.</div>
          ) : (
            <MediaGrid
              items={videos}
              onItemClick={(v) => navigate('/videos')}
              renderItem={(v) => (
                <div className="video-thumb-card">
                  <div className="video-thumb-placeholder">
                    <Film size={20} />
                  </div>
                  <div className="video-thumb-name">
                    {v.title || v.original_filename || v.id.slice(0, 8)}
                  </div>
                  {v.duration_seconds && (
                    <div className="video-thumb-duration">{formatDuration(v.duration_seconds)}</div>
                  )}
                </div>
              )}
            />
          )}
        </section>
      )}
    </div>
  );
}

function formatDuration(seconds) {
  const s = Math.floor(seconds);
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  const sec = s % 60;
  if (h > 0) return `${h}:${String(m).padStart(2, '0')}:${String(sec).padStart(2, '0')}`;
  return `${m}:${String(sec).padStart(2, '0')}`;
}
