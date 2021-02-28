import { flow } from 'fp-ts/function';
import { format, parseISO } from 'date-fns/fp';

export const parseAndFormatDate = flow(parseISO, format('yyyy/M/d H:mm'));
