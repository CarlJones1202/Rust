import { useState, useEffect } from 'react';
import { Video } from 'lucide-react';
import Lightbox from 'yet-another-react-lightbox';
import 'yet-another-react-lightbox/styles.css';
import { listVideos, videoUrl } from '../api';
import MediaGrid from '../components/MediaGrid';
import VideoCard from '../components/VideoCard';
import Pagination from '../components/Pagination';
import EnhancedVideoPlayer from '../components/EnhancedVideoPlayer';
import './VideosPage.css';

export default function VideosPage() {
  const [data, setData] = useState(null);
  const [page, setPage] = useState(1);
  const [loading, setLoading] = useState(true);
  const [lightboxIndex, setLightboxIndex] = useState(-1);

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

  const videos = data?.data || [];
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
        render={{
          slide: ({ slide }) => {
            if (slide.type === 'custom-video') {
              return <EnhancedVideoPlayer video={slide.video} />;
            }
            return undefined;
          },
        }}
      />
    </div>
  );
}
