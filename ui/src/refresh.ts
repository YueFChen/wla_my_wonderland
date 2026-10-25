/** Compare official UTC+8 refresh periods; retain the local-day fallback for legacy metadata. */
export function needsCollect(updatedAt: number, now: Date = new Date(), refreshHour?: number | null): boolean {
  if (!updatedAt) {
    return true
  }
  if (refreshHour != null && Number.isInteger(refreshHour) && refreshHour >= 0 && refreshHour < 24) {
    // The centers advertise refresh hours in UTC+8, independently of the visiting device.
    const offset = (8 - refreshHour) * 60 * 60 * 1000
    const day = (milliseconds: number) => Math.floor((milliseconds + offset) / 86_400_000)
    return day(updatedAt * 1000) !== day(now.getTime())
  }
  const fetched = new Date(updatedAt * 1000)
  return (
    fetched.getFullYear() !== now.getFullYear() ||
    fetched.getMonth() !== now.getMonth() ||
    fetched.getDate() !== now.getDate()
  )
}
