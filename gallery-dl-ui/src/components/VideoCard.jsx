import { Play } from 'lucide-react';
import { videoUrl } from '../api';
import './VideoCard.css';

export default function VideoCard({ video }) {
  const src = videoUrl(video.hash, video.extension);

  return (
    <div className="video-card-inner">
      <video src={src} muted preload="metadata" />
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
