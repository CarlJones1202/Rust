import React, { useRef, useState, useEffect } from 'react';
import { Play, Pause, Volume2, VolumeX, Maximize } from 'lucide-react';
import { saveVideoProgress, getVideoProgress, trickplayUrl } from '../api';
import './EnhancedVideoPlayer.css';

export default function EnhancedVideoPlayer({ video }) {
  const videoRef = useRef(null);
  const [isPlaying, setIsPlaying] = useState(false);
  const [progress, setProgress] = useState(0);
  const [volume, setVolume] = useState(1);
  const [isMuted, setIsMuted] = useState(false);
  const [showTrickplay, setShowTrickplay] = useState(false);
  const [trickplayPos, setTrickplayPos] = useState({ x: 0, y: 0, time: 0 });
  const [lastPosition, setLastPosition] = useState(0);

  useEffect(() => {
    // Load last position
    getVideoProgress(video.id).then(data => {
      if (data && data.position_seconds) {
        setLastPosition(data.position_seconds);
        if (videoRef.current) {
          videoRef.current.currentTime = data.position_seconds;
        }
      }
    });

    // Cleanup on unmount
    return () => {
      if (videoRef.current) {
        saveVideoProgress(video.id, videoRef.current.currentTime);
      }
    };
  }, [video.id]);

  useEffect(() => {
    const v = videoRef.current;
    if (!v) return;

    const handleTimeUpdate = () => {
      setProgress((v.currentTime / v.duration) * 100);
      // Periodically save progress every 5 seconds
      if (Math.floor(v.currentTime) % 5 === 0) {
        saveVideoProgress(video.id, v.currentTime);
      }
    };

    v.addEventListener('timeupdate', handleTimeUpdate);
    return () => v.removeEventListener('timeupdate', handleTimeUpdate);
  }, [video.id]);

  const togglePlay = () => {
    if (videoRef.current.paused) {
      videoRef.current.play();
      setIsPlaying(true);
    } else {
      videoRef.current.pause();
      setIsPlaying(false);
    }
  };

  const handleSeek = (e) => {
    const rect = e.target.getBoundingClientRect();
    const pos = (e.clientX - rect.left) / rect.width;
    videoRef.current.currentTime = pos * videoRef.current.duration;
  };

  const handleMouseMove = (e) => {
    const rect = e.target.getBoundingClientRect();
    const x = e.clientX - rect.left;
    const pos = x / rect.width;
    const time = pos * (video.duration_seconds || videoRef.current.duration);

    // Calculate sprite coordinates (10x10 grid)
    const index = Math.floor(pos * 100);
    const row = Math.floor(index / 10);
    const col = index % 10;

    setTrickplayPos({
      x: x,
      spriteX: col * 160,
      spriteY: row * 90, // Assuming 16:9 aspect ratio for preview
      time: time
    });
    setShowTrickplay(true);
  };

  const formatTime = (seconds) => {
    if (!seconds) return '0:00';
    const h = Math.floor(seconds / 3600);
    const m = Math.floor((seconds % 3600) / 60);
    const s = Math.floor(seconds % 60);
    if (h > 0) return `${h}:${m.toString().padStart(2, '0')}:${s.toString().padStart(2, '0')}`;
    return `${m}:${s.toString().padStart(2, '0')}`;
  };

  return (
    <div className="enhanced-player">
      <video
        ref={videoRef}
        src={video.src}
        onClick={togglePlay}
        onPlay={() => setIsPlaying(true)}
        onPause={() => setIsPlaying(false)}
        className="main-video"
      />

      <div className="controls">
        <div 
          className="progress-container"
          onClick={handleSeek}
          onMouseMove={handleMouseMove}
          onMouseLeave={() => setShowTrickplay(false)}
        >
          <div className="progress-bar">
            <div className="progress-fill" style={{ width: `${progress}%` }} />
          </div>
          
          {showTrickplay && (
            <div className="trickplay-preview" style={{ left: trickplayPos.x }}>
              <div 
                className="trickplay-image"
                style={{
                  backgroundImage: `url(${trickplayUrl(video.hash)})`,
                  backgroundPosition: `-${trickplayPos.spriteX}px -${trickplayPos.spriteY}px`
                }}
              />
              <div className="trickplay-time">{formatTime(trickplayPos.time)}</div>
            </div>
          )}
        </div>

        <div className="controls-row">
          <button onClick={togglePlay} className="control-btn">
            {isPlaying ? <Pause size={20} fill="currentColor" /> : <Play size={20} fill="currentColor" />}
          </button>
          
          <div className="time-display">
            {formatTime(videoRef.current?.currentTime)} / {formatTime(video.duration_seconds || videoRef.current?.duration)}
          </div>

          <div className="spacer" />

          <button onClick={() => setIsMuted(!isMuted)} className="control-btn">
            {isMuted ? <VolumeX size={20} /> : <Volume2 size={20} />}
          </button>
          
          <button onClick={() => videoRef.current.requestFullscreen()} className="control-btn">
            <Maximize size={20} />
          </button>
        </div>
      </div>
    </div>
  );
}
