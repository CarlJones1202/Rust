import { useState, useEffect, useCallback } from 'react';
import { useNavigate, useSearchParams } from 'react-router-dom';
import { LayoutGrid, Image, Wand2, RefreshCw } from 'lucide-react';
import { listGalleries, getGallery, imageUrl, thumbnailUrl, retroactiveUpdateTitles } from '../api';
import MediaGrid from '../components/MediaGrid';
import Pagination from '../components/Pagination';
import StatusBadge from '../components/StatusBadge';
import './GalleriesPage.css';

export default function GalleriesPage() {
  const [data, setData] = useState(null);
  const [searchParams, setSearchParams] = useSearchParams();
  const page = parseInt(searchParams.get('page') || '1', 10);
  const [loading, setLoading] = useState(true);
  const [galleryCoverCache, setGalleryCoverCache] = useState({});
  const [reguessing, setReguessing] = useState(false);
  const [reguessResult, setReguessResult] = useState(null);
  const navigate = useNavigate();

  const handlePageChange = (newPage) => {
    const params = new URLSearchParams(searchParams);
    if (newPage > 1) params.set('page', String(newPage));
    else params.delete('page');
    setSearchParams(params);
  };

  const fetchGalleries = useCallback(() => {
    setLoading(true);
    listGalleries(page, 24)
      .then((res) => {
        setData(res);
        // Fetch first image for each gallery as cover
        res.data.forEach((gallery) => {
          if (!galleryCoverCache[gallery.id]) {
            getGallery(gallery.id).then((detail) => {
              if (detail.images && detail.images.length > 0) {
                setGalleryCoverCache((prev) => ({
                  ...prev,
                  [gallery.id]: detail.images[0],
                }));
              }
            });
          }
        });
      })
      .catch(console.error)
      .finally(() => setLoading(false));
  }, [page, galleryCoverCache]);

  useEffect(() => {
    fetchGalleries();
  }, [fetchGalleries]);

  const handleReguess = async (force) => {
    setReguessing(true);
    setReguessResult(null);
    try {
      const res = await retroactiveUpdateTitles(force);
      setReguessResult({
        type: 'success',
        text: `Updated ${res.requests_updated} request(s), ${res.galleries_updated} gallery(ies), ${res.videos_updated} video(s).`,
      });
      fetchGalleries();
    } catch (err) {
      setReguessResult({ type: 'error', text: err.message || 'Failed to re-guess titles' });
    } finally {
      setReguessing(false);
    }
  };

  if (loading && !data) {
    return <div className="empty-state"><p>Loading...</p></div>;
  }

  return (
    <div>
      <div className="page-header">
        <h2>Galleries</h2>
        <p>Collections of images from downloaded content</p>
        <div className="page-header-actions">
          <button
            className="btn btn-ghost"
            onClick={() => handleReguess(false)}
            disabled={reguessing}
            title="Guess titles for any untitled galleries and requests"
          >
            {reguessing ? <RefreshCw size={14} className="spin" /> : <Wand2 size={14} />}
            Re-guess Titles
          </button>
          <button
            className="btn btn-ghost"
            onClick={() => {
              if (window.confirm('Re-guess ALL titles, overwriting existing ones?')) {
                handleReguess(true);
              }
            }}
            disabled={reguessing}
            title="Re-guess every title, overwriting existing values"
          >
            Force Re-guess
          </button>
        </div>
        {reguessResult && (
          <p
            className={`reguess-message ${reguessResult.type}`}
            style={{
              marginTop: '10px',
              fontSize: '0.875rem',
              color: reguessResult.type === 'success' ? 'var(--success)' : 'var(--error)',
            }}
          >
            {reguessResult.text}
          </p>
        )}
      </div>

      {data?.data.length === 0 ? (
        <div className="empty-state">
          <LayoutGrid size={48} />
          <h3>No galleries yet</h3>
          <p>Galleries are created when URLs containing images are downloaded</p>
        </div>
      ) : (
        <>
          <MediaGrid
            items={data?.data || []}
            onItemClick={(gallery) => navigate(`/galleries/${gallery.id}`)}
            renderItem={(gallery) => {
              const cover = galleryCoverCache[gallery.id];
              return (
                <div className="gallery-card-inner">
                  {cover ? (
                    <img
                      src={thumbnailUrl(cover.hash)}
                      alt={gallery.title || 'Gallery'}
                      loading="lazy"
                    />
                  ) : (
                    <span className="gallery-placeholder">
                      <Image size={32} />
                    </span>
                  )}
                  <div className="overlay">
                    <div className="overlay-text">
                      {gallery.title || `Gallery ${gallery.id.slice(0, 8)}`}
                    </div>
                  </div>
                  <div className="gallery-card-status">
                    <StatusBadge status={gallery.status} />
                  </div>
                </div>
              );
            }}
          />
          {data?.pagination && (
            <Pagination
              page={data.pagination.page}
              totalPages={data.pagination.total_pages}
              total={data.pagination.total}
              onPageChange={handlePageChange}
            />
          )}
        </>
      )}
    </div>
  );
}
