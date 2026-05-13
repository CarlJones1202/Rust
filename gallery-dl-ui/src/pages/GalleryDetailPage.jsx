import { useState, useEffect } from 'react';
import { useParams, Link } from 'react-router-dom';
import { ArrowLeft, Image, Edit2, Check, X } from 'lucide-react';
import Lightbox from 'yet-another-react-lightbox';
import 'yet-another-react-lightbox/styles.css';
import { getGallery, imageUrl, thumbnailUrl, updateGallery } from '../api';
import MediaGrid from '../components/MediaGrid';
import './GalleryDetailPage.css';

export default function GalleryDetailPage() {
  const { id } = useParams();
  const [gallery, setGallery] = useState(null);
  const [loading, setLoading] = useState(true);
  const [lightboxIndex, setLightboxIndex] = useState(-1);
  const [isEditing, setIsEditing] = useState(false);
  const [editTitle, setEditTitle] = useState('');

  useEffect(() => {
    setLoading(true);
    getGallery(id)
      .then((data) => {
        setGallery(data);
        setEditTitle(data.title || '');
      })
      .catch(console.error)
      .finally(() => setLoading(false));
  }, [id]);

  const handleTitleUpdate = async () => {
    if (!editTitle.trim()) return;
    try {
      const updated = await updateGallery(id, editTitle.trim());
      setGallery({ ...gallery, title: updated.title });
      setIsEditing(false);
    } catch (err) {
      alert(`Failed to update title: ${err.message}`);
    }
  };

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
        {isEditing ? (
          <div className="title-edit-group">
            <input
              type="text"
              value={editTitle}
              onChange={(e) => setEditTitle(e.target.value)}
              className="title-input"
              autoFocus
              onKeyDown={(e) => {
                if (e.key === 'Enter') handleTitleUpdate();
                if (e.key === 'Escape') setIsEditing(false);
              }}
            />
            <button className="btn btn-primary btn-icon" onClick={handleTitleUpdate}>
              <Check size={16} />
            </button>
            <button className="btn btn-ghost btn-icon" onClick={() => setIsEditing(false)}>
              <X size={16} />
            </button>
          </div>
        ) : (
          <div className="title-display-group">
            <h2>{gallery.title || `Gallery ${gallery.id.slice(0, 8)}`}</h2>
            <button className="btn btn-ghost btn-icon" onClick={() => setIsEditing(true)}>
              <Edit2 size={16} />
            </button>
          </div>
        )}
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
                src={thumbnailUrl(img.hash, img.extension)}
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
