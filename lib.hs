{-# LANGUAGE OverloadedStrings #-}
module Main where
import Analysis
import Serialization
import Timeline
import Data.Aeson (FromJSON, eitherDecode, encode, object, (.=))
import Data.ByteString.Lazy.Char8 (pack, unpack)
import Foreign.C.String (CString, newCString, peekCString)
import Foreign.C.Types (CFloat(..), CInt(..))
import Foreign.Marshal.Alloc (free)

decodeJSON :: FromJSON a => String -> Either String a
decodeJSON = eitherDecode . pack

hs_scoreEvidence :: CString -> CString -> IO CFloat
hs_scoreEvidence evidence suspect = do
  ev <- peekCString evidence
  sus <- peekCString suspect
  let score = case decodeJSON ev of
        Left _ -> 0
        Right parsed -> scoreEvidence parsed sus
  pure (realToFrac score)
foreign export ccall hs_scoreEvidence :: CString -> CString -> IO CFloat

hs_scoreAlibi :: CString -> IO CFloat
hs_scoreAlibi alibi = do
  al <- peekCString alibi
  let score = case decodeJSON al of
        Left _ -> 0
        Right parsed -> scoreAlibi parsed
  pure (realToFrac score)
foreign export ccall hs_scoreAlibi :: CString -> IO CFloat

hs_calculateGuiltProbability :: CString -> CString -> CString -> IO CFloat
hs_calculateGuiltProbability suspect evidence alibis = do
  sus <- peekCString suspect
  evStr <- peekCString evidence
  alStr <- peekCString alibis
  let evid = case decodeJSON evStr of
        Left _ -> []
        Right (xs :: [Evidence]) -> xs
  let albs = case decodeJSON alStr of
        Left _ -> []
        Right (ys :: [Alibi]) -> ys
  pure . realToFrac $ calculateGuiltProbability sus evid albs
foreign export ccall hs_calculateGuiltProbability :: CString -> CString -> CString -> IO CFloat

hs_findContradictions :: CString -> CString -> IO CString
hs_findContradictions alibis evidence = do
  alStr <- peekCString alibis
  evStr <- peekCString evidence
  let albs = case decodeJSON alStr of
        Left _ -> []
        Right (xs :: [Alibi]) -> xs
  let evs = case decodeJSON evStr of
        Left _ -> []
        Right (ys :: [Evidence]) -> ys
  let payload = encode (map contradictionValue (findContradictions albs evs))
  newCString (unpack payload)
foreign export ccall hs_findContradictions :: CString -> CString -> IO CString

contradictionValue (a,b,str) = object ["suspectA" .= a, "suspectB" .= b, "strength" .= str]

hs_buildDagVisualization :: CString -> IO CString
hs_buildDagVisualization payload = do
  input <- peekCString payload
  case readFromJSON input of
    Left err -> newCString err
    Right (albs, evs) -> newCString (dagVisualize (buildDAG albs evs))
foreign export ccall hs_buildDagVisualization :: CString -> IO CString

hs_canCommit :: CString -> CString -> CString -> CString -> IO CInt
hs_canCommit suspect range alibis evidence = do
  sus <- peekCString suspect
  rangeStr <- peekCString range
  alStr <- peekCString alibis
  evStr <- peekCString evidence
  let verdict = case (decodeJSON rangeStr, decodeJSON alStr, decodeJSON evStr) of
        (Right rng, Right (als :: [Alibi]), Right (evs :: [Evidence])) -> canCommit sus rng als evs
        _ -> Nothing
  pure $
    case verdict of
      Just True -> 1
      Just False -> 0
      Nothing -> -1
foreign export ccall hs_canCommit :: CString -> CString -> CString -> CString -> IO CInt

hs_loadTimelineFile :: CString -> IO CString
hs_loadTimelineFile path = do
  filepath <- peekCString path
  tl <- loadTimeline filepath
  newCString (toJSON tl)
foreign export ccall hs_loadTimelineFile :: CString -> CString -> IO CString

hs_calculateGuilt :: CString -> IO CFloat
hs_calculateGuilt suspect = do
  sus <- peekCString suspect
  pure . realToFrac $ calculateGuilt sus
foreign export ccall hs_calculateGuilt :: CString -> IO CFloat

hs_freeCString :: CString -> IO ()
hs_freeCString = free
foreign export ccall hs_freeCString :: CString -> IO ()

main :: IO ()
main = pure ()
