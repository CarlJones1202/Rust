import { useState, useEffect } from 'react';
import { useParams, Link, useNavigate } from 'react-router-dom';
import {
  ArrowLeft, Edit2, Check, X, Users, Globe, User,
  Calendar, Ruler, Hash, Info, ExternalLink, Image as ImageIcon,
  Trash2, Star, Upload, Link as LinkIcon, ChevronLeft, ChevronRight, Plus
} from 'lucide-react';
import { 
  getPerson, updatePerson, personImageUrl, thumbnailUrl,
  uploadPersonImage, deletePersonImage, setPersonPrimaryImage,
  deletePerson, listPersonGalleries, unlinkGalleryPerson, importFromStashDB,
  relinkPerson
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
      setEditData({ ...person, aliases: aliases || [] });

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
      // Aliases must always be sent as an array (even if empty) when editing,
      // so the backend replaces them rather than leaving them untouched.
      payload.aliases = editData.aliases || [];

      const updated = await updatePerson(id, payload);
      setData(updated);
      setIsEditing(false);
    } catch (err) {
      alert(`Failed to update: ${err.message}`);
    }
  };

  const addAlias = () => {
    const next = [...(editData.aliases || []), ''];
    setEditData({ ...editData, aliases: next });
  };

  const updateAlias = (index, value) => {
    const next = [...(editData.aliases || [])];
    next[index] = value;
    setEditData({ ...editData, aliases: next });
  };

  const removeAlias = (index) => {
    const next = [...(editData.aliases || [])];
    next.splice(index, 1);
    setEditData({ ...editData, aliases: next });
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

  const handleRelink = async () => {
    if (!window.confirm('Scan completed downloads and auto-link galleries matching this person\'s name or aliases?')) return;
    try {
      const result = await relinkPerson(id);
      alert(`Linked ${result.linked} gallery(ies) to this person`);
      loadData();
    } catch (err) {
      alert(`Failed to relink: ${err.message}`);
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
            <button className="btn btn-secondary" onClick={handleRelink}>
            <LinkIcon size={16} />
            Relink Galleries
          </button>
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
            {person.disambiguation && !isEditing && <p className="disambiguation">{person.disambiguation}</p>}
          </div>

          <div className="alias-section">
            <div className="section-label">Aliases</div>
            {isEditing ? (
              <div className="alias-editor">
                {(editData.aliases || []).map((alias, idx) => (
                  <div key={idx} className="alias-edit-row">
                    <input
                      type="text"
                      value={alias}
                      onChange={(e) => updateAlias(idx, e.target.value)}
                      placeholder="Alias name"
                      className="alias-input"
                    />
                    <button
                      type="button"
                      className="btn-icon btn-icon-sm"
                      onClick={() => removeAlias(idx)}
                      title="Remove alias"
                    >
                      <X size={12} />
                    </button>
                  </div>
                ))}
                <button
                  type="button"
                  className="btn btn-ghost btn-sm"
                  onClick={addAlias}
                >
                  <Plus size={14} /> Add alias
                </button>
              </div>
            ) : (
              <div className="alias-list">
                {aliases.length > 0 ? aliases.map(a => <span key={a} className="alias-tag">{a}</span>) : <span className="text-muted">No aliases</span>}
              </div>
            )}
          </div>

          <div className="quick-stats">
             <div className="stat-item">
                <ImageIcon size={16} />
                <span>{images.length} Photos</span>
             </div>
             <div className="stat-item">
                <Users size={16} />
                <span>{gallery_count} Galleries</span>
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
                <MetadataItem icon={<Ruler size={16}/>} label="Height" value={person.height ? `${person.height} cm` : null} isEditing={isEditing} onChange={v => setEditData({...editData, height: v === '' ? null : parseInt(v)})} type="number" />
                <MetadataItem icon={<Calendar size={16}/>} label="Career" value={person.career_start_year ? `${person.career_start_year} - ${person.career_end_year || 'Present'}` : null} isEditing={isEditing} isRange startValue={editData.career_start_year} endValue={editData.career_end_year} onChangeStart={v => setEditData({...editData, career_start_year: v === '' ? null : parseInt(v)})} onChangeEnd={v => setEditData({...editData, career_end_year: v === '' ? null : parseInt(v)})} />
                <MetadataItem icon={<Hash size={16}/>} label="Measurements" value={person.measurements} isEditing={isEditing} onChange={v => setEditData({...editData, measurements: v})} />
                {isEditing && (
                  <MetadataItem icon={<Info size={16}/>} label="Disambiguation" value={person.disambiguation} isEditing={isEditing} onChange={v => setEditData({...editData, disambiguation: v})} />
                )}
                <MetadataItem icon={<User size={16}/>} label="Hair Color" value={person.hair_color} isEditing={isEditing} onChange={v => setEditData({...editData, hair_color: v})} />
                <MetadataItem icon={<User size={16}/>} label="Eye Color" value={person.eye_color} isEditing={isEditing} onChange={v => setEditData({...editData, eye_color: v})} />
                <MetadataItem icon={<User size={16}/>} label="Breast Type" value={person.breast_type} isEditing={isEditing} onChange={v => setEditData({...editData, breast_type: v})} />
                <MetadataItem icon={<Hash size={16}/>} label="StashDB ID" value={person.stashdb_id} isEditing={false} />
              </div>
              {isEditing && (
                <div className="meta-item" style={{ marginTop: '12px' }}>
                  <div className="meta-label"><Info size={16} /><span>Extra Data</span></div>
                  <div className="meta-value">
                    <textarea
                      className="extra-data-textarea"
                      value={JSON.stringify(editData.extra_data || {}, null, 2)}
                      onChange={e => {
                        const raw = e.target.value;
                        try {
                          setEditData({...editData, extra_data: JSON.parse(raw)});
                        } catch {
                          // Keep the string as-is for editing, backend will handle errors
                          setEditData({...editData, extra_data: raw});
                        }
                      }}
                      rows={5}
                      placeholder="{}"
                    />
                  </div>
                </div>
              )}
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
                       {g.cover_hash ? (
                         <img src={thumbnailUrl(g.cover_hash)} alt={g.title || 'Gallery'} className="gallery-thumb-img" loading="lazy" />
                       ) : (
                         <div className="gallery-thumb-placeholder"><ImageIcon size={18} /></div>
                       )}
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

        <aside className="grid-sidebar">
           <div className="quick-actions-panel">
             <h3>Quick Actions</h3>
             <button className="btn btn-secondary btn-block" onClick={handleRelink}>
               <LinkIcon size={14} /> Relink Galleries
             </button>
             <button className="btn btn-secondary btn-block" onClick={() => setShowStashModal(true)}>
               <ExternalLink size={14} /> StashDB Import
             </button>
             {person.stashdb_id && (
               <a
                 className="btn btn-ghost btn-block"
                 href={`https://stashdb.org/performers/${person.stashdb_id}`}
                 target="_blank"
                 rel="noopener noreferrer"
               >
                 <ExternalLink size={14} /> View on StashDB
               </a>
             )}
           </div>
        </aside>
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

function MetadataItem({ icon, label, value, isEditing, onChange, isRange, onChangeStart, onChangeEnd, startValue, endValue, type = "text" }) {
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
              <input type="number" placeholder="Start" value={startValue ?? ''} onChange={e => onChangeStart(e.target.value)} />
              <span>-</span>
              <input type="number" placeholder="End" value={endValue ?? ''} onChange={e => onChangeEnd(e.target.value)} />
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
