import { useState, useEffect } from 'react';
import { Link } from 'react-router-dom';
import { Image, Info } from 'lucide-react';
import Lightbox from 'yet-another-react-lightbox';
import Captions from 'yet-another-react-lightbox/plugins/captions';
import 'yet-another-react-lightbox/styles.css';
import 'yet-another-react-lightbox/plugins/captions.css';
import { listImages, imageUrl, thumbnailUrl } from '../api';
import MediaGrid from '../components/MediaGrid';
import Pagination from '../components/Pagination';
import './ImagesPage.css';

export default function ImagesPage() {
  const [data, setData] = useState(null);
  const [page, setPage] = useState(1);
  const [loading, setLoading] = useState(true);
  const [lightboxIndex, setLightboxIndex] = useState(-1);
  const [showMetadata, setShowMetadata] = useState(true);

  useEffect(() => {
    setLoading(true);
    listImages(page, 48)
      .then(setData)
      .catch(console.error)
      .finally(() => setLoading(false));
  }, [page]);

  if (loading && !data) {
    return <div className="empty-state"><p>Loading...</p></div>;
  }

  const images = data?.data || [];
  const slides = images.map((img) => ({
    src: imageUrl(img.hash, img.extension),
    title: img.original_filename || `${img.hash}.${img.extension}`,
    description: showMetadata ? (
      <div className="lightbox-metadata">
        <div className="metadata-row">
          <span className="metadata-label">Dimensions</span>
          <span className="metadata-value">{img.width && img.height ? `${img.width} x ${img.height}` : 'Unknown'}</span>
        </div>
        <div className="metadata-row">
          <span className="metadata-label">Size</span>
          <span className="metadata-value">{(img.file_size_bytes / 1024 / 1024).toFixed(2)} MB</span>
        </div>
        {img.top_colors && (
          <div className="metadata-row">
            <span className="metadata-label">Colors</span>
            <div className="color-palette">
              {JSON.parse(img.top_colors).map(c => (
                <div key={c} className="color-swatch" style={{ backgroundColor: c }} title={c} />
              ))}
            </div>
          </div>
        )}
        {img.gallery_id && (
          <div className="gallery-link-info">
            <div className="metadata-row">
              <span className="metadata-label">Gallery</span>
              <Link to={`/galleries/${img.gallery_id}`} className="metadata-value" onClick={() => setLightboxIndex(-1)}>
                {img.gallery_title || `Gallery ${img.gallery_id.slice(0, 8)}`}
              </Link>
            </div>
          </div>
        )}
      </div>
    ) : null,
  }));

  return (
    <div>
      <div className="page-header">
        <h2>Images</h2>
        <p>All downloaded images across all galleries</p>
      </div>

      {images.length === 0 ? (
        <div className="empty-state">
          <Image size={48} />
          <h3>No images yet</h3>
          <p>Images will appear here after downloading URLs with image content</p>
        </div>
      ) : (
        <>
          <MediaGrid
            items={images}
            onItemClick={(_, index) => setLightboxIndex(index)}
            renderItem={(img) => (
              <>
                <img
                  src={thumbnailUrl(img.hash)}
                  alt={img.original_filename || ''}
                  loading="lazy"
                />
                <div className="overlay">
                  <div className="overlay-text">
                    {img.original_filename || `${img.hash}.${img.extension}`}
                  </div>
                </div>
              </>
            )}
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

      <Lightbox
        open={lightboxIndex >= 0}
        index={lightboxIndex}
        close={() => setLightboxIndex(-1)}
        slides={slides}
        controller={{ closeOnBackdropClick: true }}
        plugins={[Captions]}
        captions={{ descriptionTextAlign: 'left' }}
        render={{
          button: ({ type, label, onClick }) => {
            if (type === "info") {
              return (
                <button
                  type="button"
                  className="yarl__button"
                  title="Toggle Metadata"
                  onClick={() => setShowMetadata(!showMetadata)}
                >
                  <Info size={24} style={{ opacity: showMetadata ? 1 : 0.5 }} />
                </button>
              );
            }
          }
        }}
        toolbar={{
          buttons: ["info", "close"]
        }}
      />
    </div>
  );
}
