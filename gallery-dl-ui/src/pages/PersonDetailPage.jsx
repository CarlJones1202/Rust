import { useState, useEffect } from 'react';
import { useParams, Link, useNavigate } from 'react-router-dom';
import { 
  ArrowLeft, Edit2, Check, X, Users, Globe, User, 
  Calendar, Ruler, Hash, Info, ExternalLink, Image as ImageIcon,
  Trash2, Star, Upload, Link as LinkIcon, ChevronLeft, ChevronRight
} from 'lucide-react';
import { 
  getPerson, updatePerson, personImageUrl, thumbnailUrl,
  uploadPersonImage, deletePersonImage, setPersonPrimaryImage,
  deletePerson, listPersonGalleries, unlinkGalleryPerson, importFromStashDB
} from '../api';
import MediaGrid from '../components/MediaGrid';
import StashDBSearchModal from '../components/StashDBSearchModal';
import './PersonDetailPage.css';

export default function PersonDetailPage() {
  const { id } = useParams();
  const navigate = useNavigate();
  const [data, setData] = useState(null);
  const [galleries, setGalleries] = useState([]);
  const [loading, setLoading] = useState(true);
  const [isEditing, setIsEditing] = useState(false);
  const [editData, setEditData] = useState({});
  const [showStashModal, setShowStashModal] = useState(false);
  const [currentPhotoIndex, setCurrentPhotoIndex] = useState(0);

  useEffect(() => {
    loadData();
  }, [id]);

  const loadData = async () => {
    setLoading(true);
    try {
      const personData = await getPerson(id);
      const { aliases, images, gallery_count, ...person } = personData;
      setData(personData);
      setEditData(person);
      
      const galleryData = await listPersonGalleries(id);
      setGalleries(galleryData);
    } catch (err) {
      console.error(err);
    } finally {
      setLoading(false);
    }
  };

  const handleUpdate = async () => {
    try {
      // Clean up empty strings to null for the backend
      const payload = { ...editData };
      Object.keys(payload).forEach(key => {
        if (payload[key] === '') payload[key] = null;
      });
      
      const updated = await updatePerson(id, payload);
      setData(updated);
      setIsEditing(false);
    } catch (err) {
      alert(`Failed to update: ${err.message}`);
    }
  };

  const handleDeletePerson = async () => {
    if (!window.confirm('Are you sure you want to delete this person? This cannot be undone.')) return;
    try {
      await deletePerson(id);
      navigate('/people');
    } catch (err) {
      alert(`Failed to delete: ${err.message}`);
    }
  };

  const handleImageUpload = async (e) => {
    const file = e.target.files[0];
    if (!file) return;
    try {
      await uploadPersonImage(id, file);
      loadData();
    } catch (err) {
      alert(`Upload failed: ${err.message}`);
    }
  };

  const handleSetPrimary = async (imageId) => {
    try {
      await setPersonPrimaryImage(id, imageId);
      loadData();
    } catch (err) {
      alert(err.message);
    }
  };

  const handleDeleteImage = async (imageId) => {
    if (!window.confirm('Delete this image?')) return;
    try {
      await deletePersonImage(id, imageId);
      loadData();
      setCurrentPhotoIndex(0);
    } catch (err) {
      alert(err.message);
    }
  };

  const handleUnlinkGallery = async (galleryId) => {
    if (!window.confirm('Unlink this gallery from this person?')) return;
    try {
      await unlinkGalleryPerson(id, galleryId);
      loadData();
    } catch (err) {
      alert(err.message);
    }
  };

  if (loading && !data) return <div className="empty-state"><p>Loading...</p></div>;
  if (!data) return <div className="empty-state"><h3>Person not found</h3></div>;

  const { aliases, images, gallery_count, ...person } = data;
  const currentImage = images[currentPhotoIndex] || images[0];

  const handleNextPhoto = (e) => {
    e.stopPropagation();
    setCurrentPhotoIndex((currentPhotoIndex + 1) % images.length);
  };

  const handlePrevPhoto = (e) => {
    e.stopPropagation();
    setCurrentPhotoIndex((currentPhotoIndex - 1 + images.length) % images.length);
  };

  return (
    <div className="person-detail">
      <div className="detail-nav">
        <Link to="/people" className="back-link">
          <ArrowLeft size={16} />
          Back to People
        </Link>
        <div className="nav-actions">
           <button className="btn btn-secondary" onClick={() => setShowStashModal(true)}>
            <ExternalLink size={16} />
            StashDB Import
          </button>
          <button className="btn btn-danger" onClick={handleDeletePerson}>
            <Trash2 size={16} />
          </button>
        </div>
      </div>

      <div className="person-header">
        <div className="header-left">
          <div className="profile-image-container carousel">
            {currentImage ? (
              <>
                <img 
                  src={personImageUrl(currentImage.hash, currentImage.extension)} 
                  alt={person.name} 
                />
                {images.length > 1 && (
                  <>
                    <button className="carousel-nav prev" onClick={handlePrevPhoto}><ChevronLeft size={24} /></button>
                    <button className="carousel-nav next" onClick={handleNextPhoto}><ChevronRight size={24} /></button>
                  </>
                )}
                <div className="photo-actions-overlay">
                   <button 
                     title="Set as primary" 
                     className={`action-btn ${currentImage.is_primary ? 'active' : ''}`}
                     onClick={() => handleSetPrimary(currentImage.id)}
                   >
                     <Star size={16} />
                   </button>
                   <button 
                     title="Delete photo" 
                     className="action-btn delete"
                     onClick={() => handleDeleteImage(currentImage.id)}
                   >
                     <Trash2 size={16} />
                   </button>
                </div>
              </>
            ) : (
              <div className="profile-placeholder"><User size={64} /></div>
            )}
            <label className="upload-overlay">
              <Upload size={20} />
              <input type="file" onChange={handleImageUpload} hidden accept="image/*" />
            </label>
          </div>
        </div>

        <div className="header-right">
          <div className="title-section">
            {isEditing ? (
              <div className="edit-row">
                <input 
                  type="text" 
                  value={editData.name || ''} 
                  onChange={e => setEditData({...editData, name: e.target.value})}
                  className="title-input-large"
                />
                <button className="btn btn-primary" onClick={handleUpdate}><Check size={20} /></button>
                <button className="btn btn-ghost" onClick={() => setIsEditing(false)}><X size={20} /></button>
              </div>
            ) : (
              <div className="display-row">
                <h2>{person.name}</h2>
                <button className="btn btn-ghost" onClick={() => setIsEditing(true)}><Edit2 size={18} /></button>
              </div>
            )}
            {person.disambiguation && <p className="disambiguation">{person.disambiguation}</p>}
          </div>

          <div className="alias-section">
            <div className="section-label">Aliases</div>
            <div className="alias-list">
              {aliases.length > 0 ? aliases.map(a => <span key={a} className="alias-tag">{a}</span>) : <span className="text-muted">No aliases</span>}
            </div>
          </div>

          <div className="quick-stats">
             <div className="stat-item">
                <ImageIcon size={16} />
                <span>{images.length} Photos</span>
             </div>
             <div className="stat-item">
                <Users size={16} />
                <span>{galleries.length} Galleries</span>
             </div>
          </div>
        </div>
      </div>

      <div className="detail-grid">
        <div className="grid-main">
           <section className="metadata-section">
              <h3>Metadata</h3>
              <div className="metadata-grid">
                <MetadataItem icon={<Globe size={16}/>} label="Country" value={person.country} isEditing={isEditing} onChange={v => setEditData({...editData, country: v})} />
                <MetadataItem icon={<Users size={16}/>} label="Gender" value={person.gender} isEditing={isEditing} onChange={v => setEditData({...editData, gender: v})} />
                <MetadataItem icon={<Info size={16}/>} label="Ethnicity" value={person.ethnicity} isEditing={isEditing} onChange={v => setEditData({...editData, ethnicity: v})} />
                <MetadataItem icon={<Ruler size={16}/>} label="Height" value={person.height ? `${person.height} cm` : null} isEditing={isEditing} onChange={v => setEditData({...editData, height: parseInt(v)})} type="number" />
                <MetadataItem icon={<Calendar size={16}/>} label="Career" value={person.career_start_year ? `${person.career_start_year} - ${person.career_end_year || 'Present'}` : null} isEditing={isEditing} isRange onChangeStart={v => setEditData({...editData, career_start_year: parseInt(v)})} onChangeEnd={v => setEditData({...editData, career_end_year: parseInt(v)})} />
                <MetadataItem icon={<Hash size={16}/>} label="Measurements" value={person.measurements} isEditing={isEditing} onChange={v => setEditData({...editData, measurements: v})} />
              </div>
           </section>

           <section className="bio-section">
              <h3>Biography</h3>
              {isEditing ? (
                <textarea 
                  className="bio-textarea"
                  value={editData.bio || ''}
                  onChange={e => setEditData({...editData, bio: e.target.value})}
                  placeholder="Write something about them..."
                />
              ) : (
                <p className="bio-text">{person.bio || 'No biography provided.'}</p>
              )}
           </section>

           <section className="galleries-section">
              <div className="section-header">
                <h3>Linked Galleries</h3>
                <Link to="/galleries" className="btn btn-ghost btn-sm">Manage Links</Link>
              </div>
              {galleries.length > 0 ? (
                <MediaGrid 
                  items={galleries}
                  renderItem={(g) => (
                    <div className="gallery-thumb-card">
                       <div className="gallery-thumb-name">{g.title || g.id.slice(0, 8)}</div>
                       <button className="unlink-btn" onClick={(e) => { e.stopPropagation(); handleUnlinkGallery(g.id); }}>
                          <X size={14} />
                       </button>
                    </div>
                  )}
                  onItemClick={(g) => navigate(`/galleries/${g.id}`)}
                />
              ) : (
                <div className="empty-substate">No galleries linked to this person.</div>
              )}
           </section>
        </div>

        <div className="grid-sidebar">
           {/* Sidebar now only contains high-level info or empty for future usage */}
        </div>
      </div>

      {showStashModal && (
        <StashDBSearchModal 
          personName={person.name}
          onClose={() => setShowStashModal(false)}
          onImport={async (stashId) => {
            const updated = await importFromStashDB(id, stashId);
            setData(updated);
            setShowStashModal(false);
          }}
        />
      )}
    </div>
  );
}

function MetadataItem({ icon, label, value, isEditing, onChange, isRange, onChangeStart, onChangeEnd, type = "text" }) {
  return (
    <div className="meta-item">
      <div className="meta-label">
        {icon}
        <span>{label}</span>
      </div>
      <div className="meta-value">
        {isEditing ? (
          isRange ? (
            <div className="range-inputs">
              <input type="number" placeholder="Start" onChange={e => onChangeStart(e.target.value)} />
              <span>-</span>
              <input type="number" placeholder="End" onChange={e => onChangeEnd(e.target.value)} />
            </div>
          ) : (
            <input type={type} value={value || ''} onChange={e => onChange(e.target.value)} />
          )
        ) : (
          <span>{value || '—'}</span>
        )}
      </div>
    </div>
  );
}
