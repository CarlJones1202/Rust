import { useState, useEffect } from 'react';
import { useNavigate } from 'react-router-dom';
import { LayoutGrid, Image } from 'lucide-react';
import { listGalleries, getGallery, imageUrl, thumbnailUrl } from '../api';
import MediaGrid from '../components/MediaGrid';
import Pagination from '../components/Pagination';
import './GalleriesPage.css';

export default function GalleriesPage() {
  const [data, setData] = useState(null);
  const [page, setPage] = useState(1);
  const [loading, setLoading] = useState(true);
  const [galleryCoverCache, setGalleryCoverCache] = useState({});
  const navigate = useNavigate();

  useEffect(() => {
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
  }, [page]);

  if (loading && !data) {
    return <div className="empty-state"><p>Loading...</p></div>;
  }

  return (
    <div>
      <div className="page-header">
        <h2>Galleries</h2>
        <p>Collections of images from downloaded content</p>
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
                      src={thumbnailUrl(cover.hash, cover.extension)}
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
                </div>
              );
            }}
          />
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
