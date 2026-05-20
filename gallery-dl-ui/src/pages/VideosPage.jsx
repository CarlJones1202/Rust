import { useState, useEffect } from 'react';
import { useSearchParams } from 'react-router-dom';
import { Video, Edit2, Check, X } from 'lucide-react';
import Lightbox from 'yet-another-react-lightbox';
import 'yet-another-react-lightbox/styles.css';
import { listVideos, videoUrl, updateVideo } from '../api';
import MediaGrid from '../components/MediaGrid';
import VideoCard from '../components/VideoCard';
import Pagination from '../components/Pagination';
import EnhancedVideoPlayer from '../components/EnhancedVideoPlayer';
import './VideosPage.css';

export default function VideosPage() {
  const [data, setData] = useState(null);
  const [searchParams, setSearchParams] = useSearchParams();
  const page = parseInt(searchParams.get('page') || '1', 10);
  const [loading, setLoading] = useState(true);
  const [lightboxIndex, setLightboxIndex] = useState(-1);
  const [editingVideoId, setEditingVideoId] = useState(null);
  const [editTitle, setEditTitle] = useState('');

  const handlePageChange = (newPage) => {
    const params = new URLSearchParams(searchParams);
    if (newPage > 1) params.set('page', String(newPage));
    else params.delete('page');
    setSearchParams(params);
  };

  useEffect(() => {
    setLoading(true);
    listVideos(page, 24)
      .then(setData)
      .catch(console.error)
      .finally(() => setLoading(false));
  }, [page]);

  if (loading && !data) {
    return <div className="empty-state"><p>Loading...</p></div>;
  }

  const handleEditStart = (video) => {
    setEditingVideoId(video.id);
    setEditTitle(video.title || '');
  };

  const handleEditSave = async (id) => {
    if (!editTitle.trim()) return;
    try {
      const updated = await updateVideo(id, editTitle.trim());
      setData(prev => ({
        ...prev,
        data: prev.data.map(v => v.id === id ? { ...v, title: updated.title } : v),
      }));
      setEditingVideoId(null);
    } catch (err) {
      alert(`Failed to update title: ${err.message}`);
    }
  };

  const handleEditCancel = () => {
    setEditingVideoId(null);
    setEditTitle('');
  };

  const videos = data?.data || [];
  const currentVideo = lightboxIndex >= 0 ? videos[lightboxIndex] : null;
  const slides = videos.map((vid) => ({
    type: 'custom-video',
    video: {
      ...vid,
      src: videoUrl(vid.hash, vid.extension)
    },
    src: videoUrl(vid.hash, vid.extension),
    sources: [
      {
        src: videoUrl(vid.hash, vid.extension),
        type: `video/${vid.extension === 'mkv' ? 'x-matroska' : vid.extension}`,
      },
    ],
  }));

  return (
    <div>
      <div className="page-header">
        <h2>Videos</h2>
        <p>All downloaded videos</p>
      </div>

      {videos.length === 0 ? (
        <div className="empty-state">
          <Video size={48} />
          <h3>No videos yet</h3>
          <p>Videos will appear here after downloading URLs with video content</p>
        </div>
      ) : (
        <>
          <MediaGrid
            items={videos}
            large
            onItemClick={(_, index) => setLightboxIndex(index)}
            renderItem={(vid) => <VideoCard video={vid} />}
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

      <Lightbox
        open={lightboxIndex >= 0}
        index={lightboxIndex}
        close={() => setLightboxIndex(-1)}
        slides={slides}
        render={{
          slide: ({ slide, offset }) => {
            if (slide.type === 'custom-video') {
              const svid = slide.video;
              const isEditing = editingVideoId === svid.id;
              return (
                <div className="lightbox-video-wrapper">
                  <EnhancedVideoPlayer video={svid} autoPlay={offset === 0} />
                  {isEditing ? (
                    <div className="video-title-edit">
                      <input
                        type="text"
                        value={editTitle}
                        onChange={e => setEditTitle(e.target.value)}
                        className="title-input"
                        autoFocus
                        onKeyDown={e => {
                          if (e.key === 'Enter') handleEditSave(svid.id);
                          if (e.key === 'Escape') handleEditCancel();
                        }}
                      />
                      <button className="btn btn-primary btn-icon" onClick={() => handleEditSave(svid.id)}>
                        <Check size={16} />
                      </button>
                      <button className="btn btn-ghost btn-icon" onClick={handleEditCancel}>
                        <X size={16} />
                      </button>
                    </div>
                  ) : (
                    <div className="video-title-bar">
                      <span className="video-title-text">{svid.title || svid.original_filename || `${svid.hash}.${svid.extension}`}</span>
                      <button className="btn btn-ghost btn-icon" onClick={() => handleEditStart(svid)}>
                        <Edit2 size={14} />
                      </button>
                    </div>
                  )}
                </div>
              );
            }
            return undefined;
          },
        }}
      />
    </div>
  );
}
