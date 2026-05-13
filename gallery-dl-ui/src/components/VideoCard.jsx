import { Play } from 'lucide-react';
import { videoUrl, thumbnailUrl } from '../api';
import './VideoCard.css';

export default function VideoCard({ video }) {
  const src = videoUrl(video.hash, video.extension);
  const poster = thumbnailUrl(video.hash);

  return (
    <div className="video-card-inner">
      <video 
        src={src} 
        poster={poster}
        muted 
        preload="metadata" 
      />
      {video.progress_seconds > 0 && video.duration_seconds > 0 && (
        <div className="video-progress-overlay">
          <div 
            className="video-progress-bar" 
            style={{ width: `${Math.min(100, (video.progress_seconds / video.duration_seconds) * 100)}%` }} 
          />
        </div>
      )}
      <div className="video-play-icon">
        <Play size={22} fill="#fff" />
      </div>
      <span className="video-ext-label">{video.extension}</span>
      <div className="overlay">
        <div className="overlay-text">{video.original_filename || `${video.hash}.${video.extension}`}</div>
      </div>
    </div>
  );
}
