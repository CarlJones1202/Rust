import './MediaGrid.css';

export default function MediaGrid({ items, onItemClick, renderItem, large = false }) {
  return (
    <div className={`media-grid ${large ? 'large' : ''}`}>
      {items.map((item, index) => (
        <div
          key={item.id || index}
          className="media-grid-item"
          onClick={() => onItemClick?.(item, index)}
        >
          {renderItem(item, index)}
        </div>
      ))}
    </div>
  );
}
