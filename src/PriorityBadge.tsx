import { PRIORITY, type Priority } from './model';
export default function PriorityBadge({value}:{value?:Priority|null}) {
 return value?<span className={`priority-badge priority-${value}`} title={PRIORITY[value]} aria-label={`${value} ${PRIORITY[value]}`}>{value}</span>:null;
}
