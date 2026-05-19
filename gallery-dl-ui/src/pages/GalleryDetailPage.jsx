import { useState, useEffect } from 'react';
import { useParams, Link } from 'react-router-dom';
import { ArrowLeft, Image, Edit2, Check, X, Users, Plus, User, Info } from 'lucide-react';
import Lightbox from 'yet-another-react-lightbox';
import Captions from 'yet-another-react-lightbox/plugins/captions';
import 'yet-another-react-lightbox/styles.css';
import 'yet-another-react-lightbox/plugins/captions.css';
import { getGallery, imageUrl, thumbnailUrl, updateGallery } from '../api';
import MediaGrid from '../components/MediaGrid';
import PersonLinkModal from '../components/PersonLinkModal';
import './GalleryDetailPage.css';

export default function GalleryDetailPage() {
  const { id } = useParams();
  const [gallery, setGallery] = useState(null);
  const [loading, setLoading] = useState(true);
  const [lightboxIndex, setLightboxIndex] = useState(-1);
  const [isEditing, setIsEditing] = useState(false);
  const [editTitle, setEditTitle] = useState('');
  const [showLinkModal, setShowLinkModal] = useState(false);
  const [showMetadata, setShowMetadata] = useState(false);

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
        <div className="gallery-link-info">
          <div className="metadata-row">
            <span className="metadata-label">Gallery</span>
            <span className="metadata-value">{gallery.title || `Gallery ${gallery.id.slice(0, 8)}`}</span>
          </div>
        </div>
      </div>
    ) : null,
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

      <div className="gallery-people-section">
        <div className="section-header">
          <div className="header-title">
            <Users size={18} />
            <h3>People</h3>
          </div>
          <button className="btn btn-ghost btn-sm" onClick={() => setShowLinkModal(true)}>
            <Plus size={16} />
            Link Person
          </button>
        </div>
        <div className="people-chips">
          {gallery.persons && gallery.persons.length > 0 ? (
            gallery.persons.map(p => (
              <Link key={p.id} to={`/people/${p.id}`} className="person-chip">
                {p.image_hash ? (
                  <img src={thumbnailUrl(p.image_hash)} alt="" />
                ) : (
                  <div className="chip-placeholder"><User size={12} /></div>
                )}
                <span>{p.name}</span>
              </Link>
            ))
          ) : (
            <span className="text-muted text-sm">No people linked to this gallery</span>
          )}
        </div>
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
      )}

      <Lightbox
        open={lightboxIndex >= 0}
        index={lightboxIndex}
        close={() => setLightboxIndex(-1)}
        slides={slides}
        controller={{ closeOnBackdropClick: true }}
        plugins={[Captions]}
        captions={{ descriptionTextAlign: 'left' }}
        toolbar={{
          buttons: [
            <button
              key="metadata-toggle"
              type="button"
              className="yarl__button"
              title="Toggle Metadata"
              onClick={() => setShowMetadata(!showMetadata)}
            >
              <Info size={24} style={{ opacity: showMetadata ? 1 : 0.5 }} />
            </button>,
            "close",
          ]
        }}
      />

      {showLinkModal && (
        <PersonLinkModal
          galleryId={id}
          onClose={() => setShowLinkModal(false)}
          onLink={() => {
            setShowLinkModal(false);
            getGallery(id).then(setGallery);
          }}
        />
      )}
    </div>
  );
}
