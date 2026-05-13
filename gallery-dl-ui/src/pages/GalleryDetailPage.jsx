import { useState, useEffect } from 'react';
import { useParams, Link } from 'react-router-dom';
import { ArrowLeft, Image } from 'lucide-react';
import Lightbox from 'yet-another-react-lightbox';
import 'yet-another-react-lightbox/styles.css';
import { getGallery, imageUrl } from '../api';
import MediaGrid from '../components/MediaGrid';
import './GalleryDetailPage.css';

export default function GalleryDetailPage() {
  const { id } = useParams();
  const [gallery, setGallery] = useState(null);
  const [loading, setLoading] = useState(true);
  const [lightboxIndex, setLightboxIndex] = useState(-1);

  useEffect(() => {
    setLoading(true);
    getGallery(id)
      .then(setGallery)
      .catch(console.error)
      .finally(() => setLoading(false));
  }, [id]);

  if (loading) {
    return <div className="empty-state"><p>Loading...</p></div>;
  }

  if (!gallery) {
    return <div className="empty-state"><h3>Gallery not found</h3></div>;
  }

  const images = gallery.images || [];
  const slides = images.map((img) => ({
    src: imageUrl(img.hash, img.extension),
    alt: img.original_filename || `${img.hash}.${img.extension}`,
  }));

  return (
    <div>
      <Link to="/galleries" className="back-link">
        <ArrowLeft size={16} />
        Back to galleries
      </Link>
      <div className="gallery-detail-header">
        <h2>{gallery.title || `Gallery ${gallery.id.slice(0, 8)}`}</h2>
      </div>
      <div className="gallery-stats">
        {images.length} image{images.length !== 1 ? 's' : ''} · Created{' '}
        {new Date(gallery.created_at + 'Z').toLocaleString()}
      </div>

      {images.length === 0 ? (
        <div className="empty-state">
          <Image size={48} />
          <h3>No images in this gallery</h3>
        </div>
      ) : (
        <MediaGrid
          items={images}
          onItemClick={(_, index) => setLightboxIndex(index)}
          renderItem={(img) => (
            <>
              <img
                src={imageUrl(img.hash, img.extension)}
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
      )}

      <Lightbox
        open={lightboxIndex >= 0}
        index={lightboxIndex}
        close={() => setLightboxIndex(-1)}
        slides={slides}
      />
    </div>
  );
}
